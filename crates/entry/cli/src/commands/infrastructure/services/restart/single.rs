use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::ProcessCleanup;

use super::super::types::RestartOutput;

pub async fn execute_api(config: &CliConfig) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting API Server");
    }

    let port = super::get_api_port();
    let Some(pid) = ProcessCleanup::check_port(port) else {
        if !quiet {
            CliService::warning("API server is not running");
            CliService::info("Starting API server...");
        }
        super::super::serve::execute(true, false, config).await?;
        let output = RestartOutput {
            service_type: "api".to_owned(),
            service_name: None,
            restarted_count: 1,
            failed_count: 0,
            message: "API server started (was not running)".to_owned(),
        };
        return Ok(CommandResult::card(output).with_title("Restart API Server"));
    };

    if !quiet {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
    }

    ProcessCleanup::terminate_gracefully(pid, 100).await;
    ProcessCleanup::kill_port(port, pid);

    ProcessCleanup::wait_for_port_free(port, 5, 500).await?;

    if !quiet {
        CliService::success("API server stopped");
        CliService::info("Starting API server...");
    }

    super::super::serve::execute(true, false, config).await?;

    let message = "API server restarted successfully".to_owned();
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "api".to_owned(),
        service_name: None,
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart API Server"))
}

pub async fn execute_agent(
    ctx: &Arc<AppContext>,
    agent: &str,
    config: &CliConfig,
) -> Result<CommandResult<RestartOutput>> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section(&format!("Restarting Agent: {}", agent));
    }

    let orchestrator = super::create_orchestrator(ctx).await?;
    let name = super::resolve_name(agent).await?;
    let service_id = orchestrator.restart_agent(&name, None).await?;

    let message = format!(
        "Agent {} restarted successfully (service ID: {})",
        agent, service_id
    );
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "agent".to_owned(),
        service_name: Some(agent.to_owned()),
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

    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )
    .context("Failed to initialize MCP manager")?;

    if build {
        manager
            .build_and_restart_services(Some(server_name.to_owned()))
            .await?;
    } else {
        manager
            .restart_services_sync(Some(server_name.to_owned()))
            .await?;
    }

    let message = format!("MCP server {} restarted successfully", server_name);
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "mcp".to_owned(),
        service_name: Some(server_name.to_owned()),
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandResult::card(output).with_title("Restart MCP Server"))
}
