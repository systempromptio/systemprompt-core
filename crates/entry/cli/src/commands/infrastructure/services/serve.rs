use crate::cli_settings::CliConfig;
use crate::interactive::{Prompter, confirm_optional};
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, ServiceCategory, validate_system};
use systemprompt_scheduler::ProcessCleanup;
use systemprompt_traits::{ModuleInfo, Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::{get_api_addr, get_api_port};

#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    pub foreground: bool,
    pub kill_port_process: bool,
    pub run_migrations: bool,
}

pub async fn execute_with_events(
    prompter: &dyn Prompter,
    options: ServeOptions,
    config: &CliConfig,
    events: Option<&StartupEventSender>,
) -> Result<String> {
    let ServeOptions {
        foreground,
        kill_port_process,
        run_migrations,
    } = options;
    let port = get_api_port();

    if events.is_none() {
        CliService::startup_banner(Some("Starting services..."));
    }

    ensure_port_free(prompter, port, kill_port_process, config, events).await?;

    register_modules(events);

    let early = bind_early(foreground, events).await?;

    let ctx = Arc::new(
        AppContext::builder()
            .with_startup_warnings(true)
            .with_migrations(run_migrations)
            .build()
            .await
            .context("Failed to initialize application context")?,
    );

    if events.is_none() {
        CliService::phase_success("Database schemas installed", None);
    }

    if let Some(tx) = events {
        tx.phase_started(Phase::Database);
        if let Err(e) = tx.unbounded_send(StartupEvent::DatabaseValidated) {
            tracing::debug!(error = %e, "startup event channel closed: DatabaseValidated");
        }
        tx.phase_completed(Phase::Database);
    } else {
        CliService::phase("Validation");
        CliService::phase_info("Running system validation...", None);
    }

    validate_system(&ctx)
        .await
        .context("System validation failed")?;

    if events.is_none() {
        CliService::phase_success("System validation complete", None);
    }

    if events.is_none() {
        CliService::phase("Server");
        if !foreground {
            CliService::phase_warning("Daemon mode not supported", Some("running in foreground"));
        }
    } else if let Some(tx) = events {
        tx.phase_started(Phase::ApiServer);
        if !foreground {
            tx.warning("Daemon mode not supported, running in foreground");
        }
    }

    if let Some(early) = early {
        systemprompt_api::services::server::run_server(
            Arc::unwrap_or_clone(ctx),
            events.cloned(),
            early,
        )
        .await?;
    }

    Ok(format!("http://127.0.0.1:{}", port))
}

pub async fn execute(
    prompter: &dyn Prompter,
    foreground: bool,
    kill_port_process: bool,
    config: &CliConfig,
) -> Result<()> {
    execute_with_events(
        prompter,
        ServeOptions {
            foreground,
            kill_port_process,
            run_migrations: true,
        },
        config,
        None,
    )
    .await
    .map(|_| ())
}

async fn ensure_port_free(
    prompter: &dyn Prompter,
    port: u16,
    kill_port_process: bool,
    config: &CliConfig,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if let Some(pid) = check_port_available(port) {
        if let Some(tx) = events
            && let Err(e) = tx.unbounded_send(StartupEvent::PortConflict { port, pid })
        {
            tracing::debug!(error = %e, "startup event channel closed: PortConflict");
        }
        handle_port_conflict(prompter, port, pid, kill_port_process, config, events).await?;
        if let Some(tx) = events
            && let Err(e) = tx.unbounded_send(StartupEvent::PortConflictResolved { port })
        {
            tracing::debug!(error = %e, "startup event channel closed: PortConflictResolved");
        }
    } else if let Some(tx) = events {
        tx.port_available(port);
    } else {
        CliService::phase_success(&format!("Port {} available", port), None);
    }
    Ok(())
}

async fn bind_early(
    foreground: bool,
    events: Option<&StartupEventSender>,
) -> Result<Option<systemprompt_api::services::server::EarlyServer>> {
    if !foreground {
        return Ok(None);
    }
    let addr = get_api_addr().context("Profile not initialized; cannot determine bind address")?;
    let early = systemprompt_api::services::server::bind_and_serve(&addr, events.cloned())
        .await
        .context("Failed to bind API listener")?;
    Ok(Some(early))
}

fn check_port_available(port: u16) -> Option<u32> {
    ProcessCleanup::check_port(port)
}

fn kill_process(pid: u32) {
    ProcessCleanup::kill_process(pid);
}

#[expect(
    clippy::too_many_arguments,
    reason = "port-conflict handling threads discrete CLI flags plus the prompt seam"
)]
async fn handle_port_conflict(
    prompter: &dyn Prompter,
    port: u16,
    pid: u32,
    kill_port_process: bool,
    config: &CliConfig,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if events.is_none() {
        CliService::warning(&format!("Port {} is already in use by PID {}", port, pid));
    }

    let should_kill = kill_port_process
        || confirm_optional(
            prompter,
            &format!("Kill process {} and restart?", pid),
            false,
            config,
        )?;

    if should_kill {
        if events.is_none() {
            CliService::info(&format!("Killing process {}...", pid));
        }
        kill_process(pid);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        if check_port_available(port).is_some() {
            return Err(anyhow::anyhow!(
                "Failed to free port {} after killing PID {}",
                port,
                pid
            ));
        }
        if events.is_none() {
            CliService::success(&format!("Port {} is now available", port));
        }
        return Ok(());
    }

    if config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Port {} is occupied by PID {}. Aborted by user.",
            port,
            pid
        ));
    }

    if events.is_none() {
        CliService::error(&format!("Port {} is already in use by PID {}", port, pid));
        CliService::info("Use --kill-port-process to terminate the process, or:");
        CliService::info("   - just api-rebuild    (rebuild and restart)");
        CliService::info("   - just api-nuke       (nuclear option - kill everything)");
        CliService::info(&format!(
            "   - kill {}             (manually kill the process)",
            pid
        ));
    }
    Err(anyhow::anyhow!(
        "Port {} is occupied by PID {}. Use --kill-port-process to terminate.",
        port,
        pid
    ))
}

fn register_modules(events: Option<&StartupEventSender>) {
    let api_registrations: Vec<_> =
        inventory::iter::<systemprompt_runtime::ModuleApiRegistration>().collect();

    if let Some(tx) = events {
        let modules: Vec<_> = api_registrations
            .iter()
            .map(|r| ModuleInfo {
                name: r.module_name.to_owned(),
                category: format!("{:?}", r.category),
            })
            .collect();
        tx.modules_loaded(modules.len(), modules);
    } else {
        CliService::phase_info(
            &format!("Loading {} route modules", api_registrations.len()),
            None,
        );

        for registration in &api_registrations {
            let category_name = match registration.category {
                ServiceCategory::Core => "Core",
                ServiceCategory::Agent => "Agent",
                ServiceCategory::Mcp => "Mcp",
                ServiceCategory::Meta => "Meta",
            };
            CliService::phase_success(
                registration.module_name,
                Some(&format!("{} routes", category_name)),
            );
        }
    }
}
