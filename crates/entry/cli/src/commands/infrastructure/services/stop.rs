use crate::cli_settings::CliConfig;
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

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
}

pub async fn execute(target: ServiceTarget, force: bool, _config: &CliConfig) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));

    if target.mcp {
        CliService::section("Stopping MCP Servers");
        stop_mcp_servers(&service_mgmt, force).await?;
    }

    if target.agents {
        CliService::section("Stopping Agents");
        stop_agents(&service_mgmt, force).await?;
    }

    if target.api {
        CliService::section("Stopping API Server");
        stop_api(force).await?;
    }

    CliService::success("All requested services stopped");
    Ok(())
}

async fn stop_api(force: bool) -> Result<()> {
    let port = get_api_port();

    if let Some(pid) = ProcessCleanup::check_port(port) {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
        if force {
            ProcessCleanup::kill_process(pid);
        } else {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
    }

    ProcessCleanup::kill_port(port);
    ProcessCleanup::wait_for_port_free(port, 5, 200).await?;

    CliService::success("API server stopped");
    Ok(())
}

async fn stop_agents(service_mgmt: &ServiceManagementService, force: bool) -> Result<()> {
    let agents = service_mgmt.get_services_by_type("agent").await?;

    if agents.is_empty() {
        CliService::info("No agents running");
        return Ok(());
    }

    let mut stopped = 0;
    for agent in &agents {
        CliService::info(&format!("Stopping {}...", agent.name));
        service_mgmt.stop_service(agent, force).await?;
        stopped += 1;
    }

    CliService::success(&format!("Stopped {} agents", stopped));
    Ok(())
}

async fn stop_mcp_servers(service_mgmt: &ServiceManagementService, force: bool) -> Result<()> {
    let servers = service_mgmt.get_services_by_type("mcp").await?;

    if servers.is_empty() {
        CliService::info("No MCP servers running");
        return Ok(());
    }

    let mut stopped = 0;
    for server in &servers {
        CliService::info(&format!("Stopping {}...", server.name));
        service_mgmt.stop_service(server, force).await?;
        stopped += 1;
    }

    CliService::success(&format!("Stopped {} MCP servers", stopped));
    Ok(())
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
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Stopping Agent: {}", agent_id));

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

    CliService::success(&format!("Agent {} stopped successfully", agent_id));

    Ok(())
}

pub async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _force: bool,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Stopping MCP Server: {}", server_name));

    let manager =
        McpManager::new(Arc::clone(ctx.db_pool())).context("Failed to initialize MCP manager")?;

    manager.stop_services(Some(server_name.to_string())).await?;

    CliService::success(&format!("MCP server {} stopped successfully", server_name));

    Ok(())
}
