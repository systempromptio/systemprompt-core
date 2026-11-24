//! `infra services serve` command: port reclamation and early bind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_extension::cli::Prompter;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_runtime::{ShutdownRequest, validate_system};
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::port_conflict::{PortRequest, ensure_port_free};
use super::{get_api_addr, get_api_port};

#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    pub foreground: bool,
    pub kill_port_process: bool,
    pub run_migrations: bool,
}

fn report_database_ready(events: Option<&StartupEventSender>, run_migrations: bool) {
    let Some(tx) = events else {
        if run_migrations {
            CliService::phase_success("Database schemas installed", None);
        } else {
            CliService::phase_success("Schema current (migrations skipped)", None);
        }
        CliService::phase("Validation");
        CliService::phase_info("Running system validation...", None);
        return;
    };
    tx.phase_started(Phase::Database);
    if let Err(e) = tx.unbounded_send(StartupEvent::DatabaseValidated) {
        tracing::debug!(error = %e, "startup event channel closed: DatabaseValidated");
    }
    tx.phase_completed(Phase::Database);
}

pub async fn execute_with_events(
    prompter: &dyn Prompter,
    options: ServeOptions,
    host: &dyn KernelHost,
    events: Option<&StartupEventSender>,
) -> Result<String> {
    let profile = host.profile()?;
    let ServeOptions {
        foreground,
        kill_port_process,
        run_migrations,
    } = options;
    let port = get_api_port(profile);

    if events.is_none() {
        CliService::startup_banner(Some("Starting services..."));
    }

    ensure_port_free(
        prompter,
        PortRequest {
            port,
            holder_grace: profile.server.shutdown.child_grace(),
            kill_port_process,
        },
        host.output(),
        events,
    )
    .await?;

    let shutdown = ShutdownRequest::default();
    let early = bind_early(profile, foreground, events, shutdown.clone()).await?;

    let app = Arc::new(
        host.server_builder()?
            .with_shutdown(shutdown)
            .with_migrations(run_migrations)
            .with_schema_verification(!run_migrations)
            .build()
            .await
            .context("Failed to initialize application context")?,
    );

    report_database_ready(events, run_migrations);

    validate_system(&app)
        .await
        .context("System validation failed")?;

    let role = app
        .kernel()
        .state()
        .get::<Arc<systemprompt_manifest::Config>>()?
        .role;
    if events.is_none() {
        CliService::phase_success("System validation complete", None);
        CliService::phase_info(&format!("Node role: {role}"), None);
    } else if let Some(tx) = events {
        tx.info(format!("Node role: {role}"));
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
        Box::pin(systemprompt_api::services::server::run_server(
            Arc::unwrap_or_clone(app),
            events.cloned(),
            early,
        ))
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

pub async fn execute(
    prompter: &dyn Prompter,
    flags: ServeFlags,
    host: &dyn KernelHost,
) -> Result<()> {
    let profile = host.profile()?;
    let migrate_on_boot = profile.database.installs_on_boot(profile.server.role);
    execute_with_events(
        prompter,
        ServeOptions {
            foreground: flags.foreground,
            kill_port_process: flags.kill_port_process,
            run_migrations: effective_run_migrations(flags.skip_migrate, migrate_on_boot),
        },
        host,
        None,
    )
    .await
    .map(|_| ())
}

async fn bind_early(
    profile: &Profile,
    foreground: bool,
    events: Option<&StartupEventSender>,
    shutdown: ShutdownRequest,
) -> Result<Option<systemprompt_api::services::server::EarlyServer>> {
    if !foreground {
        return Ok(None);
    }
    let addr = get_api_addr(profile);
    let timings = profile.server.shutdown;
    systemprompt_api::services::server::bind_and_serve(&addr, events.cloned(), shutdown, timings)
        .await
        .map(Some)
        .context("Failed to bind API listener")
}
