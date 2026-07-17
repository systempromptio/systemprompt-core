//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::context::CommandContext;
use crate::interactive::require_confirmation;
use crate::shared::CommandOutput;
use anyhow::Result;
use systemprompt_logging::CliService;
use systemprompt_scheduler::{
    OrphanCleanupReport, OrphanDisposition, ProcessCleanup, ServiceManagementService,
};

use super::get_api_port;
use super::types::CleanupOutput;

pub(super) async fn execute(
    yes: bool,
    dry_run: bool,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let config = &ctx.cli;
    let quiet = config.is_json_output();
    if !quiet {
        CliService::section("Cleaning Up Services");
    }

    let app = ctx.app_context().await?;
    let service_mgmt = ServiceManagementService::new(app.db_pool())?;
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
        ctx.prompter(),
        &format!("This will stop {} service(s). Continue?", service_count),
        yes,
        config,
    )?;

    let report = service_mgmt.cleanup_all_orphans(api_port).await?;
    render_cleanup_report(&report, quiet);

    let cleaned = report.services_cleaned();
    let stale_count = usize::try_from(report.stale_entries_removed).unwrap_or(usize::MAX);
    let message = format_cleanup_message(cleaned, quiet);
    let output = CleanupOutput {
        services_cleaned: cleaned,
        stale_entries_removed: stale_count,
        dry_run: false,
        message,
    };
    Ok(CommandOutput::card_value("Service Cleanup", &output))
}

pub fn render_cleanup_report(report: &OrphanCleanupReport, quiet: bool) {
    if quiet {
        return;
    }
    for outcome in &report.outcomes {
        match outcome.disposition {
            OrphanDisposition::StaleEntry => {
                CliService::info(&format!(
                    "Cleaning stale entry: {} (PID {} not running)",
                    outcome.name, outcome.pid
                ));
            },
            OrphanDisposition::Stopped => {
                CliService::info(&format!(
                    "Stopping {} (PID: {}, port: {})...",
                    outcome.name, outcome.pid, outcome.port
                ));
            },
        }
    }
    CliService::info("Stopping API server...");
    if report.stale_entries_removed > 0 {
        CliService::info(&format!(
            "Cleaned {} stale database entries",
            report.stale_entries_removed
        ));
    }
}

pub fn no_services_result(quiet: bool, dry_run: bool) -> CommandOutput {
    let message = "No running services found".to_owned();
    if !quiet {
        CliService::info(&message);
    }
    CommandOutput::card_value(
        "Service Cleanup",
        &CleanupOutput {
            services_cleaned: 0,
            stale_entries_removed: 0,
            dry_run,
            message,
        },
    )
}

pub fn dry_run_result(
    running_services: &[systemprompt_database::ServiceConfig],
    api_pid: Option<u32>,
    api_port: u16,
    quiet: bool,
) -> CommandOutput {
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
    CommandOutput::card_value(
        "Service Cleanup (Dry Run)",
        &CleanupOutput {
            services_cleaned: service_count,
            stale_entries_removed: 0,
            dry_run: true,
            message: format!("Would clean {} service(s)", service_count),
        },
    )
}

pub fn log_service_state(service: &systemprompt_database::ServiceConfig) {
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

pub fn format_cleanup_message(cleaned: usize, quiet: bool) -> String {
    if cleaned > 0 {
        let msg = format!("Cleaned up {} services", cleaned);
        if !quiet {
            CliService::success(&msg);
        }
        msg
    } else {
        let msg = "No running services found".to_owned();
        if !quiet {
            CliService::info(&msg);
        }
        msg
    }
}
