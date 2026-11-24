//! `infra services start` command resolving targets and running startup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use crate::presentation::StartupRenderer;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_extension::cli::CliConfig;
use systemprompt_identifiers::{McpServerId, ServiceName};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{StartupPlan, StartupRequest};
use systemprompt_traits::{
    Phase, ServiceProcessKind, StartupEvent, StartupEventExt, startup_channel,
};

use super::{get_api_port, lifecycle};

#[derive(Debug, Clone, Copy)]
pub struct ServiceTarget {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceFlags {
    pub all: bool,
    pub targets: ServiceTargetFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceTargetFlags {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

impl ServiceTarget {
    pub const fn all() -> Self {
        Self {
            api: true,
            agents: true,
            mcp: true,
        }
    }

    pub const fn from_flags(flags: ServiceFlags) -> Self {
        if flags.all || (!flags.targets.api && !flags.targets.agents && !flags.targets.mcp) {
            Self::all()
        } else {
            Self {
                api: flags.targets.api,
                agents: flags.targets.agents,
                mcp: flags.targets.mcp,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StartupOptions {
    pub skip_migrate: bool,
    pub kill_port_process: bool,
}

pub(super) async fn execute(
    target: ServiceTarget,
    options: StartupOptions,
    host: &dyn KernelHost,
) -> Result<()> {
    let start_time = Instant::now();

    let (tx, rx) = startup_channel();

    let renderer = StartupRenderer::new(rx);
    let render_handle = systemprompt_traits::OwnedTask::spawn("startup_renderer", renderer.run());

    let result = run_startup(&target, &options, host, &tx).await;

    if let Err(e) = &result
        && tx
            .unbounded_send(StartupEvent::StartupFailed {
                error: e.to_string(),
                duration: start_time.elapsed(),
            })
            .is_err()
    {
        tracing::debug!("Failed to send startup failed event (receiver dropped)");
    }

    drop(tx);
    if render_handle.join().await.is_err() {
        tracing::debug!("Render task panicked or was cancelled");
    }

    result.map(|_| ())
}

async fn run_startup(
    target: &ServiceTarget,
    options: &StartupOptions,
    host: &dyn KernelHost,
    events: &systemprompt_traits::StartupEventSender,
) -> Result<String> {
    let plan = StartupPlan::compute(StartupRequest {
        api: target.api,
        agents: target.agents,
        mcp: target.mcp,
        skip_migrate: options.skip_migrate,
    });

    events.phase_started(Phase::PreFlight);

    if matches!(CredentialsBootstrap::read_unvalidated(), Ok(Some(_))) {
        events.info("Cloud credentials available");
    } else {
        events.info("Running in local-only mode (no cloud sync)");
    }

    events.phase_completed(Phase::PreFlight);

    if plan.run_migrations {
        events.phase_started(Phase::Database);
        super::super::db::execute(
            super::super::db::DbCommands::Migrate {
                allow_checksum_drift: false,
                repair_drift: false,
                full: false,
            },
            host,
        )
        .await?;
        events.phase_completed(Phase::Database);
    }

    if plan.start_api {
        let api_url = super::serve::execute_with_events(
            host.prompter(),
            super::serve::ServeOptions {
                foreground: true,
                kill_port_process: options.kill_port_process,
                run_migrations: false,
            },
            host,
            Some(events),
        )
        .await?;
        return Ok(api_url);
    }

    if plan.agents_standalone_notice {
        events.phase_started(Phase::Agents);
        events.warning("Standalone agent start not supported");
        events.info("Agents are managed by the API server lifecycle");
        events.info("Use 'services start' or 'services serve' to start all services");
        events.phase_completed(Phase::Agents);
    }

    if plan.mcp_standalone_notice {
        events.phase_started(Phase::McpServers);
        events.warning("Standalone MCP server start not supported");
        events.info("MCP servers are managed by the API server lifecycle");
        events.info("Use 'services start' or 'services serve' to start all services");
        events.phase_completed(Phase::McpServers);
    }

    Ok(format!(
        "http://127.0.0.1:{}",
        get_api_port(host.profile()?)
    ))
}

pub(super) async fn execute_individual_agent(
    ctx: &Arc<AppContext>,
    agent: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting Agent: {}", agent));

    let runs = lifecycle::process_set(ctx, ServiceProcessKind::Agent)?
        .start(Some(&ServiceName::new(agent)))
        .await?;
    let service = runs.first().map(|run| run.name.clone());
    lifecycle::single_run(runs, agent)?;

    CliService::success(&format!(
        "Agent {} started successfully (service ID: {})",
        agent,
        service.map_or_else(String::new, |name| name.to_string())
    ));

    Ok(())
}

pub(super) async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &McpServerId,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting MCP Server: {}", server_name));

    lifecycle::process_set(ctx, ServiceProcessKind::McpServer)?
        .start(Some(&ServiceName::new(server_name.as_str())))
        .await?;

    CliService::success(&format!("MCP server {} started successfully", server_name));

    Ok(())
}
