//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::context::CommandContext;
use crate::shared::CommandOutput;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::ServiceManagementService;

use super::start::ServiceTarget;
use super::types::{StopIndividualOutput, StopServiceOutput};
use super::{get_api_port, lifecycle};

pub(super) async fn execute(
    target: ServiceTarget,
    force: bool,
    ctx: &CommandContext,
) -> Result<CommandOutput> {
    let config = &ctx.cli;
    let app = ctx.app_context().await?;
    let service_mgmt = ServiceManagementService::new(app.db_pool())?;

    let mcp_stopped = if target.mcp {
        if !config.is_json_output() {
            CliService::section("Stopping MCP Servers");
        }
        stop_mcp_servers(&service_mgmt, force, config.is_json_output()).await?
    } else {
        0
    };

    let agents_stopped = if target.agents {
        if !config.is_json_output() {
            CliService::section("Stopping Agents");
        }
        stop_agents(&service_mgmt, force, config.is_json_output()).await?
    } else {
        0
    };

    let api_stopped = if target.api {
        if !config.is_json_output() {
            CliService::section("Stopping API Server");
        }
        stop_api(force, config.is_json_output()).await?;
        true
    } else {
        false
    };

    let message = "All requested services stopped".to_owned();
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopServiceOutput {
        api_stopped,
        agents_stopped,
        mcp_stopped,
        message,
    };

    Ok(CommandOutput::card_value("Stop Services", &output))
}

async fn stop_api(force: bool, quiet: bool) -> Result<()> {
    let port = get_api_port();

    let stopped_pid = ServiceManagementService::stop_api_by_port(port, force).await?;
    if let Some(pid) = stopped_pid
        && !quiet
    {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
    }

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

pub(super) async fn execute_individual_agent(
    ctx: &Arc<AppContext>,
    agent: &str,
    force: bool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_json_output() {
        CliService::section(&format!("Stopping Agent: {}", agent));
    }

    let orchestrator = lifecycle::agent_orchestrator(ctx).await?;
    let name = lifecycle::resolve_agent_name(agent).await?;

    if force {
        orchestrator.delete_agent(&name).await?;
    } else {
        orchestrator.disable_agent(&name).await?;
    }

    let message = format!("Agent {} stopped successfully", agent);
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopIndividualOutput {
        service_type: "agent".to_owned(),
        service_name: agent.to_owned(),
        stopped: true,
        message,
    };

    Ok(CommandOutput::card_value("Stop Agent", &output))
}

pub(super) async fn execute_individual_mcp(
    ctx: &Arc<AppContext>,
    server_name: &str,
    _force: bool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    if !config.is_json_output() {
        CliService::section(&format!("Stopping MCP Server: {}", server_name));
    }

    let manager = lifecycle::mcp_orchestrator(ctx)?;
    manager.stop_services(Some(server_name.to_owned())).await?;

    let message = format!("MCP server {} stopped successfully", server_name);
    if !config.is_json_output() {
        CliService::success(&message);
    }

    let output = StopIndividualOutput {
        service_type: "mcp".to_owned(),
        service_name: server_name.to_owned(),
        stopped: true,
        message,
    };

    Ok(CommandOutput::card_value("Stop MCP Server", &output))
}
