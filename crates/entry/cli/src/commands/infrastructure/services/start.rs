use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::presentation::StartupRenderer;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{StartupPlan, StartupRequest};
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, startup_channel};

use super::lifecycle;

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
pub struct StartupOptions {
    pub skip_migrate: bool,
    pub kill_port_process: bool,
}

pub(super) async fn execute(
    target: ServiceTarget,
    options: StartupOptions,
    ctx: &CommandContext,
) -> Result<()> {
    let start_time = Instant::now();

    let (tx, rx) = startup_channel();

    let renderer = StartupRenderer::new(rx);
    let render_handle = tokio::spawn(renderer.run());

    let result = run_startup(&target, &options, ctx, &tx).await;

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
    if render_handle.await.is_err() {
        tracing::debug!("Render task panicked or was cancelled");
    }

    result.map(|_| ())
}

async fn run_startup(
    target: &ServiceTarget,
    options: &StartupOptions,
    ctx: &CommandContext,
    events: &systemprompt_traits::StartupEventSender,
) -> Result<String> {
    let plan = StartupPlan::compute(StartupRequest {
        api: target.api,
        agents: target.agents,
        mcp: target.mcp,
        skip_migrate: options.skip_migrate,
    });

    events.phase_started(Phase::PreFlight);

    match CredentialsBootstrap::get() {
        Ok(Some(_)) => {
            events.info("Cloud credentials available");
        },
        Ok(None) | Err(_) => {
            events.info("Running in local-only mode (no cloud sync)");
        },
    }

    events.phase_completed(Phase::PreFlight);

    if plan.run_migrations {
        events.phase_started(Phase::Database);
        super::super::db::execute(
            super::super::db::DbCommands::Migrate {
                allow_checksum_drift: false,
            },
            ctx,
        )
        .await?;
        events.phase_completed(Phase::Database);
    }

    if plan.start_api {
        let api_url = super::serve::execute_with_events(
            ctx.prompter(),
            super::serve::ServeOptions {
                foreground: true,
                kill_port_process: options.kill_port_process,
                run_migrations: false,
            },
            &ctx.cli,
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
        ProfileBootstrap::get().map_or(8080, |p| p.server.port)
    ))
}

pub(super) async fn execute_individual_agent(
    ctx: &Arc<AppContext>,
    agent: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting Agent: {}", agent));

    let orchestrator = lifecycle::agent_orchestrator(ctx).await?;
    let name = lifecycle::resolve_agent_name(agent).await?;
    let service_id = orchestrator.start_agent(&name, None).await?;

    CliService::success(&format!(
        "Agent {} started successfully (service ID: {})",
        agent, service_id
    ));

    Ok(())
}

pub(super) async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting MCP Server: {}", server_name));

    let manager = lifecycle::mcp_orchestrator(ctx)?;
    manager.start_services(Some(server_name.to_owned())).await?;

    CliService::success(&format!("MCP server {} started successfully", server_name));

    Ok(())
}
