//! Port-conflict handling for `infra services serve`: detects the process
//! holding the API port and stops it when it is verified and confirmed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::interactive::{Prompter, confirm_optional};
use anyhow::{Context, Result};
use std::time::Duration;
use systemprompt_loader::subprocess::{self, ChildKind};
use systemprompt_logging::CliService;
use systemprompt_scheduler::{port_holders, wait_for_port_free};
use systemprompt_traits::{StartupEvent, StartupEventExt, StartupEventSender};

const CONFIRMED_HOLDER_GRACE: Duration = Duration::from_secs(2);
const PORT_RELEASE: Duration = Duration::from_secs(5);

pub(super) async fn ensure_port_free(
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

#[derive(Debug, Clone, Copy)]
struct PortConflict {
    port: u16,
    pid: u32,
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
