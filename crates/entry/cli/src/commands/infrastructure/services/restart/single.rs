//! Single-service restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_extension::cli::{CliConfig, CommandOutput, Prompter};
use systemprompt_identifiers::{McpServerId, ServiceName};
use systemprompt_loader::subprocess::StopOutcome;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ServiceManagementService, port_holders};
use systemprompt_traits::{RestartMode, ServiceProcessKind};

use super::super::lifecycle;
use super::super::types::RestartOutput;

pub async fn execute_api(prompter: &dyn Prompter, host: &dyn KernelHost) -> Result<CommandOutput> {
    let config = host.output();
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting API Server");
    }

    let port = super::super::get_api_port(host.profile()?);
    let Some(pid) = port_holders(port).await?.first().copied() else {
        if !quiet {
            CliService::warning("API server is not running");
            CliService::info("Starting API server...");
        }
        super::super::serve::execute(
            prompter,
            super::super::serve::ServeFlags {
                foreground: true,
                kill_port_process: false,
                skip_migrate: false,
            },
            host,
        )
        .await?;
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

    let stops =
        ServiceManagementService::stop_api_by_port(port, &host.profile()?.server.shutdown, false)
            .await?;
    if let Some(foreign) = stops
        .iter()
        .find(|stop| stop.outcome == StopOutcome::NotOurs)
    {
        anyhow::bail!(
            "Port {port} is held by PID {}, which is not a verified systemprompt API server; it \
             was not signalled. Stop it by hand, or use `infra services serve \
             --kill-port-process`.",
            foreign.pid
        );
    }

    if !quiet {
        CliService::success("API server stopped");
        CliService::info("Starting API server...");
    }

    super::super::serve::execute(
        prompter,
        super::super::serve::ServeFlags {
            foreground: true,
            kill_port_process: false,
            skip_migrate: false,
        },
        host,
    )
    .await?;

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

    let runs = lifecycle::process_set(ctx, ServiceProcessKind::Agent)?
        .restart(Some(&ServiceName::new(agent)), RestartMode::Restart)
        .await?;
    let name = runs
        .first()
        .map_or_else(|| ServiceName::new(agent), |run| run.name.clone());
    lifecycle::single_run(runs, agent)?;

    let message = format!(
        "Agent {} restarted successfully (service ID: {})",
        agent, name
    );
    if !quiet {
        CliService::success(&message);
    }

    let output = RestartOutput {
        service_type: "agent".to_owned(),
        service_name: Some(name),
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

    let mode = if build {
        RestartMode::Rebuild
    } else {
        RestartMode::Restart
    };
    let runs = lifecycle::process_set(ctx, ServiceProcessKind::McpServer)?
        .restart(Some(&ServiceName::new(server_name.as_str())), mode)
        .await?;
    if runs.is_empty() {
        anyhow::bail!("{server_name} is not a managed MCP server");
    }
    for run in runs {
        run.outcome?;
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
