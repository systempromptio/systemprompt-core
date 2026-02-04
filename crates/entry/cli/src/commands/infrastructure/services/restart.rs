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
use systemprompt_scheduler::ProcessCleanup;

use super::types::RestartOutput;

const DEFAULT_API_PORT: u16 = 8080;

fn create_agent_state(ctx: &AppContext) -> Result<Arc<AgentState>> {
    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    Ok(Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    )))
}

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

pub async fn execute_api(config: &CliConfig) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting API Server");
    }

    let port = get_api_port();
    let Some(pid) = ProcessCleanup::check_port(port) else {
        if !quiet {
            CliService::warning("API server is not running");
            CliService::info("Starting API server...");
        }
        super::serve::execute(true, false, config).await?;
        let output = RestartOutput {
            service_type: "api".to_string(),
            service_name: None,
            restarted_count: 1,
            failed_count: 0,
            message: "API server started (was not running)".to_string(),
        };
        return Ok(CommandResult::card(output).with_title("Restart API Server"));
    };

    if !quiet {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
    }

    ProcessCleanup::terminate_gracefully(pid, 100).await;
    ProcessCleanup::kill_port(port);

    ProcessCleanup::wait_for_port_free(port, 5, 500).await?;

    if !quiet {
        CliService::success("API server stopped");
        CliService::info("Starting API server...");
    }

    super::serve::execute(true, false, config).await?;

    let message = "API server restarted successfully".to_string();
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "api".to_string(),
        service_name: None,
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart API Server"))
}

pub async fn execute_agent(
    ctx: &Arc<AppContext>,
    agent_id: &str,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section(&format!("Restarting Agent: {}", agent_id));
    }

    let agent_state = create_agent_state(ctx)?;
    let orchestrator = AgentOrchestrator::new(agent_state, None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let name = resolve_name(agent_id).await?;
    let service_id = orchestrator.restart_agent(&name, None).await?;

    let message = format!(
        "Agent {} restarted successfully (service ID: {})",
        agent_id, service_id
    );
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "agent".to_string(),
        service_name: Some(agent_id.to_string()),
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart Agent"))
}

pub async fn execute_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    build: bool,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();
    let action = if build {
        "Building and restarting"
    } else {
        "Restarting"
    };

    if !quiet {
        CliService::section(&format!("{} MCP Server: {}", action, server_name));
    }

    let manager =
        McpManager::new(Arc::clone(ctx.db_pool())).context("Failed to initialize MCP manager")?;

    if build {
        manager
            .build_and_restart_services(Some(server_name.to_string()))
            .await?;
    } else {
        manager
            .restart_services_sync(Some(server_name.to_string()))
            .await?;
    }

    let message = format!("MCP server {} restarted successfully", server_name);
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "mcp".to_string(),
        service_name: Some(server_name.to_string()),
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart MCP Server"))
}

pub async fn execute_all_agents(
    ctx: &Arc<AppContext>,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting All Agents");
    }

    let agent_state = create_agent_state(ctx)?;
    let orchestrator = AgentOrchestrator::new(agent_state, None)
        .await
        .context("Failed to initialize agent orchestrator")?;

    let agent_registry = AgentRegistry::new().await?;
    let all_agents = orchestrator.list_all().await?;

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for (agent_id, _status) in &all_agents {
        let Ok(agent_config) = agent_registry.get_agent(agent_id).await else {
            continue;
        };

        if !agent_config.enabled {
            continue;
        }

        if !quiet {
            CliService::info(&format!("Restarting agent: {}", agent_config.name));
        }
        match orchestrator.restart_agent(agent_id, None).await {
            Ok(_) => {
                restarted += 1;
                if !quiet {
                    CliService::success(&format!("  {} restarted", agent_config.name));
                }
            },
            Err(e) => {
                failed += 1;
                if !quiet {
                    CliService::error(&format!("  Failed to restart {}: {}", agent_config.name, e));
                }
            },
        }
    }

    let message = match (restarted, failed) {
        (0, 0) => {
            if !quiet {
                CliService::info("No enabled agents found");
            }
            "No enabled agents found".to_string()
        },
        (r, 0) => {
            let msg = format!("Restarted {} agents", r);
            if !quiet {
                CliService::success(&msg);
            }
            msg
        },
        (0, f) => {
            let msg = format!("Failed to restart {} agents", f);
            if !quiet {
                CliService::warning(&msg);
            }
            msg
        },
        (r, f) => {
            if !quiet {
                CliService::success(&format!("Restarted {} agents", r));
                CliService::warning(&format!("Failed to restart {} agents", f));
            }
            format!("Restarted {} agents, {} failed", r, f)
        },
    };

    let output = RestartOutput {
        service_type: "agents".to_string(),
        service_name: None,
        restarted_count: restarted,
        failed_count: failed,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart All Agents"))
}

pub async fn execute_all_mcp(
    ctx: &Arc<AppContext>,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting All MCP Servers");
    }

    let mcp_manager =
        McpManager::new(Arc::clone(ctx.db_pool())).context("Failed to initialize MCP manager")?;

    systemprompt_mcp::services::RegistryManager::validate()?;
    let servers = systemprompt_mcp::services::RegistryManager::get_enabled_servers()?;

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for server in servers {
        if !server.enabled {
            continue;
        }

        if !quiet {
            CliService::info(&format!("Restarting MCP server: {}", server.name));
        }
        match mcp_manager
            .restart_services(Some(server.name.clone()))
            .await
        {
            Ok(()) => {
                restarted += 1;
                if !quiet {
                    CliService::success(&format!("  {} restarted", server.name));
                }
            },
            Err(e) => {
                failed += 1;
                if !quiet {
                    CliService::error(&format!("  Failed to restart {}: {}", server.name, e));
                }
            },
        }
    }

    let message = match (restarted, failed) {
        (0, 0) => {
            if !quiet {
                CliService::info("No enabled MCP servers found");
            }
            "No enabled MCP servers found".to_string()
        },
        (r, 0) => {
            let msg = format!("Restarted {} MCP servers", r);
            if !quiet {
                CliService::success(&msg);
            }
            msg
        },
        (0, f) => {
            let msg = format!("Failed to restart {} MCP servers", f);
            if !quiet {
                CliService::warning(&msg);
            }
            msg
        },
        (r, f) => {
            if !quiet {
                CliService::success(&format!("Restarted {} MCP servers", r));
                CliService::warning(&format!("Failed to restart {} MCP servers", f));
            }
            format!("Restarted {} MCP servers, {} failed", r, f)
        },
    };

    let output = RestartOutput {
        service_type: "mcp".to_string(),
        service_name: None,
        restarted_count: restarted,
        failed_count: failed,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart All MCP Servers"))
}

pub async fn execute_failed(
    ctx: &Arc<AppContext>,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting Failed Services");
    }

    let mut restarted_count = 0usize;
    let mut failed_count = 0usize;

    restart_failed_agents(ctx, &mut restarted_count, &mut failed_count, quiet).await?;
    restart_failed_mcp(ctx, &mut restarted_count, &mut failed_count, quiet).await?;

    let message = if restarted_count > 0 {
        let msg = format!("Restarted {} failed services", restarted_count);
        if !quiet {
            CliService::success(&msg);
        }
        if failed_count > 0 && !quiet {
            CliService::warning(&format!("Failed to restart {} services", failed_count));
        }
        if failed_count > 0 {
            format!("{}, {} failed to restart", msg, failed_count)
        } else {
            msg
        }
    } else {
        let msg = "No failed services found".to_string();
        if !quiet {
            CliService::info(&msg);
        }
        msg
    };

    let output = RestartOutput {
        service_type: "failed".to_string(),
        service_name: None,
        restarted_count,
        failed_count,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart Failed Services"))
}

async fn restart_failed_agents(
    ctx: &Arc<AppContext>,
    restarted_count: &mut usize,
    failed_count: &mut usize,
    quiet: bool,
) -> Result<()> {
    let agent_state = create_agent_state(ctx)?;
    let orchestrator = AgentOrchestrator::new(agent_state, None)
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

        if let systemprompt_agent::services::agent_orchestration::AgentStatus::Failed { .. } =
            status
        {
            if !quiet {
                CliService::info(&format!("Restarting failed agent: {}", agent_config.name));
            }
            match orchestrator.restart_agent(agent_id, None).await {
                Ok(_) => {
                    *restarted_count += 1;
                    if !quiet {
                        CliService::success(&format!("  {} restarted", agent_config.name));
                    }
                },
                Err(e) => {
                    *failed_count += 1;
                    if !quiet {
                        CliService::error(&format!(
                            "  Failed to restart {}: {}",
                            agent_config.name, e
                        ));
                    }
                },
            }
        }
    }

    Ok(())
}

async fn restart_failed_mcp(
    ctx: &Arc<AppContext>,
    restarted_count: &mut usize,
    failed_count: &mut usize,
    quiet: bool,
) -> Result<()> {
    let mcp_manager =
        McpManager::new(Arc::clone(ctx.db_pool())).context("Failed to initialize MCP manager")?;

    systemprompt_mcp::services::RegistryManager::validate()?;
    let servers = systemprompt_mcp::services::RegistryManager::get_enabled_servers()?;

    for server in servers {
        if !server.enabled {
            continue;
        }

        let database = systemprompt_mcp::services::DatabaseManager::new(Arc::clone(ctx.db_pool()));
        let service_info = database.get_service_by_name(&server.name).await?;

        let needs_restart = match service_info {
            Some(info) => info.status != "running",
            None => true,
        };

        if needs_restart {
            if !quiet {
                CliService::info(&format!("Restarting MCP server: {}", server.name));
            }
            match mcp_manager
                .restart_services(Some(server.name.clone()))
                .await
            {
                Ok(()) => {
                    *restarted_count += 1;
                    if !quiet {
                        CliService::success(&format!("  {} restarted", server.name));
                    }
                },
                Err(e) => {
                    *failed_count += 1;
                    if !quiet {
                        CliService::error(&format!("  Failed to restart {}: {}", server.name, e));
                    }
                },
            }
        }
    }

    Ok(())
}
