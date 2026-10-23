//! `infra services serve` command: port reclamation and early bind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, ShutdownRequest, validate_system};
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::port_conflict::ensure_port_free;
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
            .with_schema_verification(!run_migrations)
            .build()
            .await
            .context("Failed to initialize application context")?,
    );

    if events.is_none() {
        if run_migrations {
            CliService::phase_success("Database schemas installed", None);
        } else {
            CliService::phase_success("Schema current (migrations skipped)", None);
        }
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
        CliService::phase_info(&format!("Node role: {}", ctx.config().role), None);
    } else if let Some(tx) = events {
        tx.info(format!("Node role: {}", ctx.config().role));
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

#[derive(Debug, Clone, Copy)]
pub struct ServeFlags {
    pub foreground: bool,
    pub kill_port_process: bool,
    pub skip_migrate: bool,
}

pub const fn effective_run_migrations(skip_flag: bool, migrate_on_boot: bool) -> bool {
    !skip_flag && migrate_on_boot
}

pub async fn execute(prompter: &dyn Prompter, flags: ServeFlags, config: &CliConfig) -> Result<()> {
    let migrate_on_boot = ProfileBootstrap::get()
        .context("Profile not initialized")?
        .database
        .migrate_on_boot;
    execute_with_events(
        prompter,
        ServeOptions {
            foreground: flags.foreground,
            kill_port_process: flags.kill_port_process,
            run_migrations: effective_run_migrations(flags.skip_migrate, migrate_on_boot),
        },
        config,
        None,
    )
    .await
    .map(|_| ())
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
    let timings = ProfileBootstrap::get()?.server.shutdown;
    systemprompt_api::services::server::bind_and_serve(&addr, events.cloned(), shutdown, timings)
        .await
        .map(Some)
        .context("Failed to bind API listener")
}
