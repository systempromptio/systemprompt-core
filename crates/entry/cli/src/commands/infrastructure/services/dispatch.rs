use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_runtime::AppContext;

use super::{RestartTarget, ServicesCommands, StartTarget, StopTarget, restart, start, stop};
use crate::cli_settings::CliConfig;
use crate::shared::render_result;

pub async fn execute(command: ServicesCommands, config: &CliConfig) -> Result<()> {
    match command {
        ServicesCommands::Start {
            target,
            all,
            api,
            agents,
            mcp,
            foreground: _,
            skip_migrate,
            kill_port_process,
        } => {
            let flags = start::ServiceFlags {
                all,
                targets: start::ServiceTargetFlags { api, agents, mcp },
            };
            let options = start::StartupOptions {
                skip_migrate,
                kill_port_process,
            };
            execute_start(target, flags, options, config).await
        },

        ServicesCommands::Stop {
            target,
            all,
            api,
            agents,
            mcp,
            force,
        } => {
            let flags = start::ServiceFlags {
                all,
                targets: start::ServiceTargetFlags { api, agents, mcp },
            };
            execute_stop(target, flags, force, config).await
        },

        ServicesCommands::Restart {
            target,
            failed,
            agents,
            mcp,
        } => execute_restart(target, failed, agents, mcp, config).await,

        ServicesCommands::Status {
            detailed,
            json,
            health,
        } => {
            let result = super::status::execute(detailed, json, health, config).await?;
            render_result(&result);
            Ok(())
        },

        ServicesCommands::Cleanup { yes, dry_run } => {
            let result = super::cleanup::execute(yes, dry_run, config).await?;
            render_result(&result);
            Ok(())
        },

        ServicesCommands::Serve {
            foreground,
            kill_port_process,
        } => super::serve::execute(foreground, kill_port_process, config).await,
    }
}

async fn execute_start(
    target: Option<StartTarget>,
    flags: start::ServiceFlags,
    options: start::StartupOptions,
    config: &CliConfig,
) -> Result<()> {
    if let Some(individual) = target {
        let ctx = Arc::new(
            AppContext::new()
                .await
                .context("Failed to initialize application context")?,
        );
        return match individual {
            StartTarget::Agent { agent } => {
                start::execute_individual_agent(&ctx, &agent, config).await
            },
            StartTarget::Mcp { server_name } => {
                start::execute_individual_mcp(&ctx, &server_name, config).await
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    start::execute(service_target, options, config).await
}

async fn execute_stop(
    target: Option<StopTarget>,
    flags: start::ServiceFlags,
    force: bool,
    config: &CliConfig,
) -> Result<()> {
    if let Some(individual) = target {
        let ctx = Arc::new(
            AppContext::new()
                .await
                .context("Failed to initialize application context")?,
        );
        return match individual {
            StopTarget::Agent { agent, force } => {
                let result = stop::execute_individual_agent(&ctx, &agent, force, config).await?;
                render_result(&result);
                Ok(())
            },
            StopTarget::Mcp { server_name, force } => {
                let result =
                    stop::execute_individual_mcp(&ctx, &server_name, force, config).await?;
                render_result(&result);
                Ok(())
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    let result = stop::execute(service_target, force, config).await?;
    render_result(&result);
    Ok(())
}

async fn execute_restart(
    target: Option<RestartTarget>,
    failed: bool,
    agents: bool,
    mcp: bool,
    config: &CliConfig,
) -> Result<()> {
    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let result = if failed {
        restart::execute_failed(&ctx, config).await?
    } else if agents {
        restart::execute_all_agents(&ctx, config).await?
    } else if mcp {
        restart::execute_all_mcp(&ctx, config).await?
    } else {
        match target {
            Some(RestartTarget::Api) => restart::execute_api(config).await?,
            Some(RestartTarget::Agent { agent }) => {
                restart::execute_agent(&ctx, &agent, config).await?
            },
            Some(RestartTarget::Mcp { server_name, build }) => {
                restart::execute_mcp(&ctx, &server_name, build, config).await?
            },
            None => {
                return Err(anyhow::anyhow!(
                    "Must specify target (api, agent, mcp) or use --failed/--agents/--mcp flag"
                ));
            },
        }
    };
    render_result(&result);
    Ok(())
}

pub fn load_service_configs() -> Result<Vec<systemprompt_scheduler::ServiceConfig>> {
    let services_config = systemprompt_loader::ConfigLoader::load()?;
    Ok(systemprompt_scheduler::ServiceConfig::list_from_manifest(
        &services_config,
    ))
}
