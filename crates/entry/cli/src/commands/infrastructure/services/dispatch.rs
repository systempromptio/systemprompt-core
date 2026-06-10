use anyhow::Result;

use super::{RestartTarget, ServicesCommands, StartTarget, StopTarget, restart, start, stop};
use crate::context::CommandContext;
use crate::shared::render_result;

pub async fn execute(command: ServicesCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
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
            execute_start(target, flags, options, ctx).await
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
            execute_stop(target, flags, force, ctx).await
        },

        ServicesCommands::Restart {
            target,
            failed,
            agents,
            mcp,
        } => execute_restart(target, failed, agents, mcp, ctx).await,

        ServicesCommands::Status {
            detailed,
            json,
            health,
        } => {
            let result = super::status::execute(detailed, json, health, config).await?;
            render_result(&result, config);
            Ok(())
        },

        ServicesCommands::Cleanup { yes, dry_run } => {
            let result = super::cleanup::execute(yes, dry_run, ctx).await?;
            render_result(&result, config);
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
    ctx: &CommandContext,
) -> Result<()> {
    if let Some(individual) = target {
        let app = ctx.app_context().await?;
        return match individual {
            StartTarget::Agent { agent } => {
                start::execute_individual_agent(app, &agent, &ctx.cli).await
            },
            StartTarget::Mcp { server_name } => {
                start::execute_individual_mcp(app, &server_name, &ctx.cli).await
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    start::execute(service_target, options, ctx).await
}

async fn execute_stop(
    target: Option<StopTarget>,
    flags: start::ServiceFlags,
    force: bool,
    ctx: &CommandContext,
) -> Result<()> {
    let config = &ctx.cli;
    if let Some(individual) = target {
        let app = ctx.app_context().await?;
        return match individual {
            StopTarget::Agent { agent, force } => {
                let result = stop::execute_individual_agent(app, &agent, force, config).await?;
                render_result(&result, config);
                Ok(())
            },
            StopTarget::Mcp { server_name, force } => {
                let result = stop::execute_individual_mcp(app, &server_name, force, config).await?;
                render_result(&result, config);
                Ok(())
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    let result = stop::execute(service_target, force, ctx).await?;
    render_result(&result, config);
    Ok(())
}

async fn execute_restart(
    target: Option<RestartTarget>,
    failed: bool,
    agents: bool,
    mcp: bool,
    ctx: &CommandContext,
) -> Result<()> {
    let config = &ctx.cli;
    let app = ctx.app_context().await?;

    let result = if failed {
        restart::execute_failed(app, config).await?
    } else if agents {
        restart::execute_all_agents(app, config).await?
    } else if mcp {
        restart::execute_all_mcp(app, config).await?
    } else {
        match target {
            Some(RestartTarget::Api) => restart::execute_api(config).await?,
            Some(RestartTarget::Agent { agent }) => {
                restart::execute_agent(app, &agent, config).await?
            },
            Some(RestartTarget::Mcp { server_name, build }) => {
                restart::execute_mcp(app, &server_name, build, config).await?
            },
            None => {
                return Err(anyhow::anyhow!(
                    "Must specify target (api, agent, mcp) or use --failed/--agents/--mcp flag"
                ));
            },
        }
    };
    render_result(&result, config);
    Ok(())
}

pub fn load_service_configs() -> Result<Vec<systemprompt_scheduler::ServiceConfig>> {
    let services_config = systemprompt_loader::ConfigLoader::load()?;
    Ok(systemprompt_scheduler::ServiceConfig::list_from_manifest(
        &services_config,
    ))
}
