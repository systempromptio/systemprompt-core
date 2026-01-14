use crate::cli_settings::CliConfig;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_core_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_core_agent::services::registry::AgentRegistry;
use systemprompt_core_logging::CliService;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_core_scheduler::ProcessCleanup;
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
}

async fn resolve_name(agent_identifier: &str) -> Result<String> {
    let registry = AgentRegistry::new().await?;
    let agent = registry.get_agent(agent_identifier).await?;
    Ok(agent.name)
}

pub async fn execute_api(config: &CliConfig) -> Result<()> {
    CliService::section("Restarting API Server");

    let port = get_api_port();

    // Check if API server is running
    let api_pid = ProcessCleanup::check_port(port);
    if api_pid.is_none() {
        CliService::warning("API server is not running");
        CliService::info("Starting API server...");
        return super::serve::execute(true, false, config).await;
    }

    let pid = api_pid.unwrap();
    CliService::info(&format!("Stopping API server (PID: {})...", pid));

    // Gracefully terminate the API server
    ProcessCleanup::terminate_gracefully(pid, 100).await;
    ProcessCleanup::kill_port(port);

    // Wait for port to be free
    ProcessCleanup::wait_for_port_free(port, 5, 500).await?;

    CliService::success("API server stopped");
    CliService::info("Starting API server...");

    // Start the API server again
    super::serve::execute(true, false, config).await?;

    CliService::success("API server restarted successfully");
    Ok(())
}

pub async fn execute_agent(
    ctx: &Arc<AppContext>,
    agent_id: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Restarting Agent: {}", agent_id));

    let orchestrator = AgentOrchestrator::new(Arc::clone(ctx), None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let name = resolve_name(agent_id).await?;
    let service_id = orchestrator.restart_agent(&name, None).await?;

    CliService::success(&format!(
        "Agent {} restarted successfully (service ID: {})",
        agent_id, service_id
    ));

    Ok(())
}

pub async fn execute_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _config: &CliConfig,
) -> Result<()> {
    CliService::section(&format!("Restarting MCP Server: {}", server_name));

    let manager = McpManager::new(Arc::clone(ctx)).context("Failed to initialize MCP manager")?;

    manager
        .restart_services(Some(server_name.to_string()))
        .await?;

    CliService::success(&format!(
        "MCP server {} restarted successfully",
        server_name
    ));

    Ok(())
}

pub async fn execute_failed(ctx: &Arc<AppContext>, _config: &CliConfig) -> Result<()> {
    CliService::section("Restarting Failed Services");

    let mut restarted_count = 0;
    let mut failed_count = 0;

    restart_failed_agents(ctx, &mut restarted_count, &mut failed_count).await?;
    restart_failed_mcp(ctx, &mut restarted_count, &mut failed_count).await?;

    if restarted_count > 0 {
        CliService::success(&format!("Restarted {} failed services", restarted_count));
    } else {
        CliService::info("No failed services found");
    }

    if failed_count > 0 {
        CliService::warning(&format!("Failed to restart {} services", failed_count));
    }

    Ok(())
}

async fn restart_failed_agents(
    ctx: &Arc<AppContext>,
    restarted_count: &mut i32,
    failed_count: &mut i32,
) -> Result<()> {
    let orchestrator = AgentOrchestrator::new(Arc::clone(ctx), None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let agent_registry = AgentRegistry::new().await?;

    let all_agents = orchestrator.list_all().await?;
    for (agent_id, status) in &all_agents {
        let Ok(agent_config) = agent_registry.get_agent(agent_id).await else {
            continue;
        };

        if !agent_config.enabled {
            continue;
        }

        if let systemprompt_core_agent::services::agent_orchestration::AgentStatus::Failed {
            ..
        } = status
        {
            CliService::info(&format!("Restarting failed agent: {}", agent_config.name));
            match orchestrator.restart_agent(agent_id, None).await {
                Ok(_) => {
                    *restarted_count += 1;
                    CliService::success(&format!("  {} restarted", agent_config.name));
                },
                Err(e) => {
                    *failed_count += 1;
                    CliService::error(&format!("  Failed to restart {}: {}", agent_config.name, e));
                },
            }
        }
    }

    Ok(())
}

async fn restart_failed_mcp(
    ctx: &Arc<AppContext>,
    restarted_count: &mut i32,
    failed_count: &mut i32,
) -> Result<()> {
    let mcp_manager =
        McpManager::new(Arc::clone(ctx)).context("Failed to initialize MCP manager")?;

    systemprompt_core_mcp::services::RegistryManager::validate()?;
    let servers = systemprompt_core_mcp::services::RegistryManager::get_enabled_servers()?;

    for server in servers {
        if !server.enabled {
            continue;
        }

        let database =
            systemprompt_core_mcp::services::DatabaseManager::new(Arc::clone(ctx.db_pool()));
        let service_info = database.get_service_by_name(&server.name).await?;

        let needs_restart = match service_info {
            Some(info) => info.status != "running",
            None => true,
        };

        if needs_restart {
            CliService::info(&format!("Restarting MCP server: {}", server.name));
            match mcp_manager
                .restart_services(Some(server.name.clone()))
                .await
            {
                Ok(()) => {
                    *restarted_count += 1;
                    CliService::success(&format!("  {} restarted", server.name));
                },
                Err(e) => {
                    *failed_count += 1;
                    CliService::error(&format!("  Failed to restart {}: {}", server.name, e));
                },
            }
        }
    }

    Ok(())
}
