use crate::cli_settings::CliConfig;
use crate::presentation::StartupRenderer;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;
use systemprompt_cloud::CredentialsBootstrap;
use systemprompt_core_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_core_agent::services::registry::AgentRegistry;
use systemprompt_core_logging::CliService;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{startup_channel, Phase, StartupEvent, StartupEventExt};

pub struct ServiceTarget {
    pub api: bool,
    pub agents: bool,
    pub mcp: bool,
}

#[derive(Clone, Copy)]
pub struct ServiceFlags {
    pub all: bool,
    pub targets: ServiceTargetFlags,
}

#[derive(Clone, Copy)]
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

pub struct StartupOptions {
    pub skip_web: bool,
    pub skip_migrate: bool,
}

pub async fn execute(
    target: ServiceTarget,
    options: StartupOptions,
    config: &CliConfig,
) -> Result<()> {
    let start_time = Instant::now();

    let (tx, rx) = startup_channel();

    let renderer = StartupRenderer::new(rx);
    let render_handle = tokio::spawn(renderer.run());

    let result = run_startup(&target, &options, config, &tx).await;

    if let Err(e) = &result {
        let _ = tx.send(StartupEvent::StartupFailed {
            error: e.to_string(),
            duration: start_time.elapsed(),
        });
    }

    drop(tx);
    let _ = render_handle.await;

    result.map(|_| ())
}

async fn run_startup(
    target: &ServiceTarget,
    options: &StartupOptions,
    config: &CliConfig,
    events: &systemprompt_traits::StartupEventSender,
) -> Result<String> {
    events.phase_started(Phase::PreFlight);

    match CredentialsBootstrap::get() {
        Ok(Some(_)) => {
            events.info("Cloud credentials valid");
        },
        Ok(None) => {
            anyhow::bail!(
                "Cloud credentials not found. Run 'systemprompt cloud login' to register."
            );
        },
        Err(e) => {
            anyhow::bail!("{}", e);
        },
    }

    if !options.skip_web {
        crate::shared::web::build_web_assets()?;
    }

    events.phase_completed(Phase::PreFlight);

    if !options.skip_migrate {
        events.phase_started(Phase::Database);
        super::super::db::execute(super::super::db::DbCommands::Migrate, config).await?;
        events.phase_completed(Phase::Database);
    }

    if target.api {
        let api_url = super::serve::execute_with_events(true, false, config, Some(events)).await?;
        return Ok(api_url);
    }

    if target.agents && !target.api {
        events.phase_started(Phase::Agents);
        events.warning("Standalone agent start not supported");
        events.info("Agents are managed by the API server lifecycle");
        events.info("Use 'services start' or 'services serve' to start all services");
        events.phase_completed(Phase::Agents);
    }

    if target.mcp && !target.api {
        events.phase_started(Phase::McpServers);
        events.warning("Standalone MCP server start not supported");
        events.info("MCP servers are managed by the API server lifecycle");
        events.info("Use 'services start' or 'services serve' to start all services");
        events.phase_completed(Phase::McpServers);
    }

    Ok(format!(
        "http://127.0.0.1:{}",
        ProfileBootstrap::get()
            .map(|p| p.server.port)
            .unwrap_or(8080)
    ))
}

async fn resolve_agent_name(agent_identifier: &str) -> Result<String> {
    let registry = AgentRegistry::new().await?;
    let agent = registry.get_agent(agent_identifier).await?;
    Ok(agent.name)
}

pub async fn execute_individual_agent(
    ctx: &Arc<AppContext>,
    agent_id: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting Agent: {}", agent_id));

    let orchestrator = AgentOrchestrator::new(Arc::clone(ctx), None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let name = resolve_agent_name(agent_id).await?;
    let service_id = orchestrator.start_agent(&name, None).await?;

    CliService::success(&format!(
        "Agent {} started successfully (service ID: {})",
        agent_id, service_id
    ));

    Ok(())
}

pub async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Starting MCP Server: {}", server_name));

    let manager = McpManager::new(Arc::clone(ctx)).context("Failed to initialize MCP manager")?;

    manager
        .start_services(Some(server_name.to_string()))
        .await?;

    CliService::success(&format!("MCP server {} started successfully", server_name));

    Ok(())
}
