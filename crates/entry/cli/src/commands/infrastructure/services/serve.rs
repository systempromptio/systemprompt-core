//! `infra services serve` command: port reclamation and early bind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::interactive::{Prompter, confirm_optional};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use systemprompt_loader::subprocess::{self, ChildKind};
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, ShutdownRequest, validate_system};
use systemprompt_scheduler::{port_holders, wait_for_port_free};
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::{get_api_addr, get_api_port};

const CONFIRMED_HOLDER_GRACE: Duration = Duration::from_secs(2);
const PORT_RELEASE: Duration = Duration::from_secs(5);

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
    if let Some(pid) = port_holder(port).await? {
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

async fn port_holder(port: u16) -> Result<Option<u32>> {
    Ok(port_holders(port).await?.first().copied())
}

async fn stop_confirmed_holder(port: u16, pid: u32) -> Result<()> {
    if port_holders(port).await?.contains(&pid) {
        subprocess::terminate_gracefully(pid, CONFIRMED_HOLDER_GRACE)
            .await
            .with_context(|| format!("Failed to stop PID {pid} holding port {port}"))?;
    }
    wait_for_port_free(port, PORT_RELEASE)
        .await
        .with_context(|| format!("Failed to free port {port} after stopping PID {pid}"))?;
    Ok(())
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

    let verified = subprocess::owns(pid, ChildKind::Api, &subprocess::api_server_service()).await;
    let should_kill = if verified {
        kill_port_process
            || confirm_optional(
                prompter,
                &format!("Stop the running API server (PID {pid}) and restart?"),
                false,
                config,
            )?
    } else if kill_port_process {
        confirm_optional(
            prompter,
            &format!(
                "PID {pid} holding port {port} is not a verified systemprompt API server. Signal \
                 PID {pid} anyway?"
            ),
            false,
            config,
        )?
    } else {
        false
    };

    if should_kill {
        if events.is_none() {
            CliService::info(&format!("Stopping process {}...", pid));
        }
        stop_confirmed_holder(port, pid).await?;
        if events.is_none() {
            CliService::success(&format!("Port {} is now available", port));
        }
        return Ok(());
    }

    if !verified {
        return Err(anyhow::anyhow!(
            "Port {port} is held by PID {pid}, which is not a verified systemprompt API server; \
             it was not signalled. Stop it by hand, or rerun interactively with \
             --kill-port-process and confirm."
        ));
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
        CliService::info("Use --kill-port-process to stop it, or:");
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
