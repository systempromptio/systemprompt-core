//! Single-service restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use crate::shared::CommandOutput;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_identifiers::{McpServerId, ServiceName};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ServiceManagementService, port_holders};

use super::super::lifecycle;
use super::super::types::RestartOutput;

pub async fn execute_api(prompter: &dyn Prompter, config: &CliConfig) -> Result<CommandOutput> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting API Server");
    }

    let port = super::get_api_port();
    let Some(pid) = port_holders(port).await?.first().copied() else {
        if !quiet {
            CliService::warning("API server is not running");
            CliService::info("Starting API server...");
        }
        super::super::serve::execute(prompter, true, false, config).await?;
        let output = RestartOutput {
            service_type: "api".to_owned(),
            service_name: None,
            restarted_count: 1,
            failed_count: 0,
            message: "API server started (was not running)".to_owned(),
        };
        return Ok(CommandOutput::card_value("Restart API Server", &output));
    };

    if !quiet {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
    }

    ServiceManagementService::stop_api_by_port(port, false).await?;

    if !quiet {
        CliService::success("API server stopped");
        CliService::info("Starting API server...");
    }

    super::super::serve::execute(prompter, true, false, config).await?;

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

    Ok(CommandOutput::card_value("Restart API Server", &output))
}

pub async fn execute_agent(
    ctx: &Arc<AppContext>,
    agent: &str,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section(&format!("Restarting Agent: {}", agent));
    }

    let orchestrator = lifecycle::agent_orchestrator(ctx).await?;
    let name = lifecycle::resolve_agent_name(agent).await?;
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
        service_name: Some(ServiceName::new(name)),
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandOutput::card_value("Restart Agent", &output))
}

pub async fn execute_mcp(
    ctx: &Arc<AppContext>,
    server_name: &McpServerId,
    build: bool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let quiet = config.is_json_output();
    let action = if build {
        "Building and restarting"
    } else {
        "Restarting"
    };

    if !quiet {
        CliService::section(&format!("{} MCP Server: {}", action, server_name));
    }

    let manager = lifecycle::mcp_orchestrator(ctx)?;

    if build {
        let restarted = manager
            .build_and_restart_services(Some(ServiceName::new(server_name.as_str())))
            .await?;
        if restarted == 0 {
            anyhow::bail!("{server_name} is not a managed MCP server");
        }
    } else {
        let outcomes = manager
            .restart_services(Some(ServiceName::new(server_name.as_str())))
            .await?;
        if outcomes.is_empty() {
            anyhow::bail!("{server_name} is not a managed MCP server");
        }
        for outcome in outcomes {
            outcome.result?;
        }
    }

    let message = format!("MCP server {} restarted successfully", server_name);
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "mcp".to_owned(),
        service_name: Some(ServiceName::new(server_name.as_str())),
        restarted_count: 1,
        failed_count: 0,
        message,
    };

    Ok(CommandOutput::card_value("Restart MCP Server", &output))
}
