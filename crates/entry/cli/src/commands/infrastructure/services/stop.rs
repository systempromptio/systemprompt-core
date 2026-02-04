use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_agent::AgentState;
use systemprompt_logging::CliService;
use systemprompt_mcp::services::McpManager;
use systemprompt_models::ProfileBootstrap;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, ServiceManagementService};

use super::start::ServiceTarget;
use super::types::{StopIndividualOutput, StopServiceOutput};

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
}

pub async fn execute(
    target: ServiceTarget,
    force: bool,
    config: &CliConfig,
) -> Result<CommandResult<StopServiceOutput>> {
    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));

    let mut api_stopped = false;
    let mut agents_stopped = 0usize;
    let mut mcp_stopped = 0usize;

    if target.mcp {
        if !config.is_json_output() {
            CliService::section("Stopping MCP Servers");
        }
        mcp_stopped = stop_mcp_servers(&service_mgmt, force, config.is_json_output()).await?;
    }

    if target.agents {
        if !config.is_json_output() {
            CliService::section("Stopping Agents");
        }
        agents_stopped = stop_agents(&service_mgmt, force, config.is_json_output()).await?;
    }

    if target.api {
        if !config.is_json_output() {
            CliService::section("Stopping API Server");
        }
        stop_api(force, config.is_json_output()).await?;
        api_stopped = true;
    }

    let message = "All requested services stopped".to_string();
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopServiceOutput {
        api_stopped,
        agents_stopped,
        mcp_stopped,
        message,
    };

    Ok(CommandResult::card(output).with_title("Stop Services"))
}

async fn stop_api(force: bool, quiet: bool) -> Result<()> {
    let port = get_api_port();

    if let Some(pid) = ProcessCleanup::check_port(port) {
        if !quiet {
            CliService::info(&format!("Stopping API server (PID: {})...", pid));
        }
        if force {
            ProcessCleanup::kill_process(pid);
        } else {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
    }

    ProcessCleanup::kill_port(port);
    ProcessCleanup::wait_for_port_free(port, 5, 200).await?;

    if !quiet {
        CliService::success("API server stopped");
    }
    Ok(())
}

async fn stop_agents(
    service_mgmt: &ServiceManagementService,
    force: bool,
    quiet: bool,
) -> Result<usize> {
    let agents = service_mgmt.get_services_by_type("agent").await?;

    if agents.is_empty() {
        if !quiet {
            CliService::info("No agents running");
        }
        return Ok(0);
    }

    let mut stopped = 0usize;
    for agent in &agents {
        if !quiet {
            CliService::info(&format!("Stopping {}...", agent.name));
        }
        service_mgmt.stop_service(agent, force).await?;
        stopped += 1;
    }

    if !quiet {
        CliService::success(&format!("Stopped {} agents", stopped));
    }
    Ok(stopped)
}

async fn stop_mcp_servers(
    service_mgmt: &ServiceManagementService,
    force: bool,
    quiet: bool,
) -> Result<usize> {
    let servers = service_mgmt.get_services_by_type("mcp").await?;

    if servers.is_empty() {
        if !quiet {
            CliService::info("No MCP servers running");
        }
        return Ok(0);
    }

    let mut stopped = 0usize;
    for server in &servers {
        if !quiet {
            CliService::info(&format!("Stopping {}...", server.name));
        }
        service_mgmt.stop_service(server, force).await?;
        stopped += 1;
    }

    if !quiet {
        CliService::success(&format!("Stopped {} MCP servers", stopped));
    }
    Ok(stopped)
}

async fn resolve_agent_name(agent_identifier: &str) -> Result<String> {
    let registry = AgentRegistry::new().await?;
    let agent = registry.get_agent(agent_identifier).await?;
    Ok(agent.name)
}

pub async fn execute_individual_agent(
    ctx: &Arc<AppContext>,
    agent_id: &str,
    force: bool,
    config: &CliConfig,
) -> Result<CommandResult<StopIndividualOutput>> {
    if !config.is_json_output() {
        CliService::section(&format!("Stopping Agent: {}", agent_id));
    }

    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    let agent_state = Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    ));
    let orchestrator = AgentOrchestrator::new(agent_state, None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let name = resolve_agent_name(agent_id).await?;

    if force {
        orchestrator.delete_agent(&name).await?;
    } else {
        orchestrator.disable_agent(&name).await?;
    }

    let message = format!("Agent {} stopped successfully", agent_id);
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopIndividualOutput {
        service_type: "agent".to_string(),
        service_name: agent_id.to_string(),
        stopped: true,
        message,
    };

    Ok(CommandResult::card(output).with_title("Stop Agent"))
}

pub async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _force: bool,
    config: &CliConfig,
) -> Result<CommandResult<StopIndividualOutput>> {
    if !config.is_json_output() {
        CliService::section(&format!("Stopping MCP Server: {}", server_name));
    }

    let manager =
        McpManager::new(Arc::clone(ctx.db_pool())).context("Failed to initialize MCP manager")?;

    manager.stop_services(Some(server_name.to_string())).await?;

    let message = format!("MCP server {} stopped successfully", server_name);
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopIndividualOutput {
        service_type: "mcp".to_string(),
        service_name: server_name.to_string(),
        stopped: true,
        message,
    };

    Ok(CommandResult::card(output).with_title("Stop MCP Server"))
}
