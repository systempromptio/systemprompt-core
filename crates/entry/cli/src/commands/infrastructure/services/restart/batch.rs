use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_logging::CliService;
use systemprompt_mcp::services::McpManager;
use systemprompt_runtime::AppContext;

use super::super::types::RestartOutput;

pub async fn execute_all_agents(
    ctx: &Arc<AppContext>,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting All Agents");
    }

    let orchestrator = super::create_orchestrator(ctx).await?;
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

    let message = super::format_batch_message("agents", restarted, failed, quiet);

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

    let message = super::format_batch_message("MCP servers", restarted, failed, quiet);

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
    let orchestrator = super::create_orchestrator(ctx).await?;
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
