//! Batch restart across MCP and agent targets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{RestartPlan, RestartScope, RestartTarget, ServiceType};

use super::super::lifecycle;
use super::super::types::RestartOutput;
use super::snapshots::{agent_snapshots, mcp_snapshots};

pub async fn execute_all_agents(
    ctx: &Arc<AppContext>,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting All Agents");
    }

    let orchestrator = lifecycle::agent_orchestrator(ctx).await?;
    let snapshots = agent_snapshots(&orchestrator).await?;
    let plan = RestartPlan::compute(RestartScope::AllAgents, &snapshots);

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for target in &plan.targets {
        if !quiet {
            CliService::info(&format!("Restarting agent: {}", target.name));
        }
        restart_agent_target(&orchestrator, target, &mut restarted, &mut failed, quiet).await;
    }

    let message = super::format_batch_message("agents", restarted, failed, quiet);

    let output = RestartOutput {
        service_type: "agents".to_owned(),
        service_name: None,
        restarted_count: restarted,
        failed_count: failed,
        message,
    };

    Ok(CommandOutput::card_value("Restart All Agents", &output))
}

pub async fn execute_all_mcp(ctx: &Arc<AppContext>, config: &CliConfig) -> Result<CommandOutput> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting All MCP Servers");
    }

    let mcp_manager = lifecycle::mcp_orchestrator(ctx)?;
    let snapshots = mcp_snapshots(ctx, false).await?;
    let plan = RestartPlan::compute(RestartScope::AllMcp, &snapshots);

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for target in &plan.targets {
        if !quiet {
            CliService::info(&format!("Restarting MCP server: {}", target.name));
        }
        restart_mcp_target(&mcp_manager, target, &mut restarted, &mut failed, quiet).await;
    }

    let message = super::format_batch_message("MCP servers", restarted, failed, quiet);

    let output = RestartOutput {
        service_type: "mcp".to_owned(),
        service_name: None,
        restarted_count: restarted,
        failed_count: failed,
        message,
    };

    Ok(CommandOutput::card_value(
        "Restart All MCP Servers",
        &output,
    ))
}

pub async fn execute_failed(ctx: &Arc<AppContext>, config: &CliConfig) -> Result<CommandOutput> {
    let quiet = config.is_json_output();

    if !quiet {
        CliService::section("Restarting Failed Services");
    }

    let handles = lifecycle::OrchestratorHandles::build(ctx).await?;

    let mut snapshots = agent_snapshots(&handles.agents).await?;
    snapshots.extend(mcp_snapshots(ctx, true).await?);
    let plan = RestartPlan::compute(RestartScope::Failed, &snapshots);

    let mut restarted_count = 0usize;
    let mut failed_count = 0usize;

    for target in &plan.targets {
        match target.service_type {
            ServiceType::Agent => {
                if !quiet {
                    CliService::info(&format!("Restarting failed agent: {}", target.name));
                }
                restart_agent_target(
                    &handles.agents,
                    target,
                    &mut restarted_count,
                    &mut failed_count,
                    quiet,
                )
                .await;
            },
            ServiceType::Mcp => {
                if !quiet {
                    CliService::info(&format!("Restarting MCP server: {}", target.name));
                }
                restart_mcp_target(
                    &handles.mcp,
                    target,
                    &mut restarted_count,
                    &mut failed_count,
                    quiet,
                )
                .await;
            },
            ServiceType::Api => {},
        }
    }

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
        let msg = "No failed services found".to_owned();
        if !quiet {
            CliService::info(&msg);
        }
        msg
    };

    let output = RestartOutput {
        service_type: "failed".to_owned(),
        service_name: None,
        restarted_count,
        failed_count,
        message,
    };

    Ok(CommandOutput::card_value(
        "Restart Failed Services",
        &output,
    ))
}

async fn restart_mcp_target(
    orchestrator: &systemprompt_mcp::services::McpOrchestrator,
    target: &RestartTarget,
    restarted: &mut usize,
    failed: &mut usize,
    quiet: bool,
) {
    let results: Vec<(String, Result<()>)> =
        match orchestrator.restart_services(Some(target.id.clone())).await {
            Ok(outcomes) if outcomes.is_empty() => {
                vec![(
                    target.name.clone(),
                    Err(anyhow!("not a managed MCP server")),
                )]
            },
            Ok(outcomes) => outcomes
                .into_iter()
                .map(|o| (o.service_name, o.result.map_err(anyhow::Error::from)))
                .collect(),
            Err(e) => vec![(target.name.clone(), Err(e.into()))],
        };
    for (name, result) in results {
        match result {
            Ok(()) => {
                *restarted += 1;
                if !quiet {
                    CliService::success(&format!("  {name} restarted"));
                }
            },
            Err(e) => {
                *failed += 1;
                if !quiet {
                    CliService::error(&format!("  Failed to restart {name}: {e:#}"));
                }
            },
        }
    }
}

async fn restart_agent_target(
    orchestrator: &AgentOrchestrator,
    target: &RestartTarget,
    restarted: &mut usize,
    failed: &mut usize,
    quiet: bool,
) {
    match orchestrator.restart_agent(&target.id, None).await {
        Ok(_) => {
            *restarted += 1;
            if !quiet {
                CliService::success(&format!("  {} restarted", target.name));
            }
        },
        Err(e) => {
            *failed += 1;
            if !quiet {
                CliService::error(&format!("  Failed to restart {}: {}", target.name, e));
            }
        },
    }
}
