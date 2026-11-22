//! Batch restart across MCP and agent targets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use systemprompt_extension::cli::{CliConfig, CommandOutput};
use systemprompt_identifiers::ServiceName;
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{RestartPlan, RestartScope, RestartTarget, ServiceType};
use systemprompt_traits::{DynServiceProcessSet, RestartMode, ServiceProcessKind};

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

    let agents = lifecycle::process_set(ctx, ServiceProcessKind::Agent)?;
    let snapshots = agent_snapshots(ctx).await?;
    let plan = RestartPlan::compute(RestartScope::AllAgents, &snapshots);

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for target in &plan.targets {
        if !quiet {
            CliService::info(&format!("Restarting agent: {}", target.name));
        }
        restart_target(&agents, target, &mut restarted, &mut failed, quiet).await;
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

    let mcp = lifecycle::process_set(ctx, ServiceProcessKind::McpServer)?;
    let snapshots = mcp_snapshots(ctx, false).await?;
    let plan = RestartPlan::compute(RestartScope::AllMcp, &snapshots);

    let mut restarted = 0usize;
    let mut failed = 0usize;

    for target in &plan.targets {
        if !quiet {
            CliService::info(&format!("Restarting MCP server: {}", target.name));
        }
        restart_target(&mcp, target, &mut restarted, &mut failed, quiet).await;
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

    let sets = lifecycle::process_sets(ctx);

    let mut snapshots = agent_snapshots(ctx).await?;
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
                restart_kind(
                    sets.of_kind(ServiceProcessKind::Agent),
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
                restart_kind(
                    sets.of_kind(ServiceProcessKind::McpServer),
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

async fn restart_kind(
    set: Option<&DynServiceProcessSet>,
    target: &RestartTarget,
    restarted: &mut usize,
    failed: &mut usize,
    quiet: bool,
) {
    if let Some(set) = set {
        restart_target(set, target, restarted, failed, quiet).await;
    } else {
        *failed += 1;
        if !quiet {
            CliService::error(&format!(
                "  Failed to restart {}: no admitted member supervises it",
                target.name
            ));
        }
    }
}

async fn restart_target(
    set: &DynServiceProcessSet,
    target: &RestartTarget,
    restarted: &mut usize,
    failed: &mut usize,
    quiet: bool,
) {
    let results: Vec<(String, Result<()>)> = match set
        .restart(Some(&ServiceName::new(target.id.as_str())), RestartMode::Restart)
        .await
    {
        Ok(runs) if runs.is_empty() => {
            vec![(
                target.name.clone(),
                Err(anyhow!("not a supervised service")),
            )]
        },
        Ok(runs) => runs
            .into_iter()
            .map(|run| (run.name.to_string(), run.outcome.map_err(anyhow::Error::from)))
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
