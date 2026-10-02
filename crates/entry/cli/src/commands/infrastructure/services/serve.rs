//! `infra services serve` command: port reclamation and early bind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::interactive::{Prompter, confirm_optional};
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, ShutdownRequest, validate_system};
use systemprompt_scheduler::ProcessCleanup;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

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

    let shutdown = ShutdownRequest::default();
    let early = bind_early(foreground, events, shutdown.clone()).await?;

    let ctx = Arc::new(
        AppContext::builder()
            .with_startup_warnings(true)
            .with_shutdown(shutdown)
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
        handle_port_conflict(
            prompter,
            PortConflict { port, pid },
            kill_port_process,
            config,
            events,
        )
        .await?;
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
    shutdown: ShutdownRequest,
) -> Result<Option<systemprompt_api::services::server::EarlyServer>> {
    if !foreground {
        return Ok(None);
    }
    let addr = get_api_addr().context("Profile not initialized; cannot determine bind address")?;
    let early =
        systemprompt_api::services::server::bind_and_serve(&addr, events.cloned(), shutdown)
            .await
            .context("Failed to bind API listener")?;
    Ok(Some(early))
}

const NO_PORT: u16 = 0;

// Why: Linux truncates a process `comm` to 15 bytes (TASK_COMM_LEN), so a
// longer executable name is only ever reported as its 15-byte prefix.
const COMM_NAME_LIMIT: usize = 15;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PortConflict {
    pub port: u16,
    pub pid: u32,
}

fn check_port_available(port: u16) -> Option<u32> {
    if port == NO_PORT {
        return None;
    }
    ProcessCleanup::check_port(port)
}

fn names_match(holder: &str, own: &str) -> bool {
    !holder.is_empty()
        && (holder == own || (holder.len() == COMM_NAME_LIMIT && own.starts_with(holder)))
}

pub(crate) fn verify_port_holder(conflict: PortConflict) -> Result<()> {
    let PortConflict { port, pid } = conflict;
    let own_exe = std::env::current_exe()
        .context("Cannot resolve the running executable to verify the port holder")?;
    let own_name = own_exe
        .file_name()
        .and_then(|name| name.to_str())
        .context("The running executable has no UTF-8 file name")?;
    let holder = ProcessCleanup::get_process_by_port(port)
        .filter(|info| info.pid == pid)
        .with_context(|| {
            format!("Cannot identify the process holding port {port}; refusing to kill PID {pid}")
        })?;
    let holder_name = std::path::Path::new(&holder.name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if names_match(holder_name, own_name) {
        return Ok(());
    }
    anyhow::bail!(
        "Port {port} is held by PID {pid} ({}), which is not a {own_name} process; refusing to \
         kill it",
        holder.name
    )
}

async fn handle_port_conflict(
    prompter: &dyn Prompter,
    conflict: PortConflict,
    kill_port_process: bool,
    config: &CliConfig,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    let PortConflict { port, pid } = conflict;
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
        verify_port_holder(conflict)?;
        if events.is_none() {
            CliService::info(&format!("Killing process {}...", pid));
        }
        if !ProcessCleanup::kill_process(pid) {
            anyhow::bail!("Failed to kill PID {pid} holding port {port}");
        }
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
        CliService::info("   - systemprompt infra services restart api");
        CliService::info("   - systemprompt infra services stop --all --force");
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
