//! `infra services` start/stop/restart dispatch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use crate::kernel::KernelHost;

use super::{RestartTarget, ServicesCommands, StartTarget, StopTarget, restart, start, stop};
use systemprompt_extension::cli::CommandOutput;
use crate::commands::shared::app_context;

pub async fn execute(command: ServicesCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
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
            execute_start(target, flags, options, host).await?;
            Ok(CommandOutput::rendered())
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
            execute_stop(target, flags, force, host).await
        },

        ServicesCommands::Restart {
            target,
            failed,
            agents,
            mcp,
        } => execute_restart(target, failed, agents, mcp, host).await,

        ServicesCommands::Status {
            detailed,
            json,
            health,
        } => {
            super::status::execute(detailed, json, health, host).await
        },

        ServicesCommands::Cleanup { yes, dry_run } => {
            super::cleanup::execute(yes, dry_run, host).await
        },

        ServicesCommands::Serve {
            foreground,
            kill_port_process,
            skip_migrate,
        } => {
            super::serve::execute(
                host.prompter(),
                super::serve::ServeFlags {
                    foreground,
                    kill_port_process,
                    skip_migrate,
                },
                host,
            )
            .await?;
            Ok(CommandOutput::rendered())
        },
    }
}

async fn execute_start(
    target: Option<StartTarget>,
    flags: start::ServiceFlags,
    options: start::StartupOptions,
    host: &dyn KernelHost,
) -> Result<()> {
    if let Some(individual) = target {
        let app = app_context(host).await?;
        return match individual {
            StartTarget::Agent { agent } => {
                start::execute_individual_agent(&app, &agent, host.output()).await
            },
            StartTarget::Mcp { server_name } => {
                start::execute_individual_mcp(&app, &server_name, host.output()).await
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    start::execute(service_target, options, host).await
}

async fn execute_stop(
    target: Option<StopTarget>,
    flags: start::ServiceFlags,
    force: bool,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let config = host.output();
    if let Some(individual) = target {
        let app = app_context(host).await?;
        return match individual {
            StopTarget::Agent { agent, force } => {
                stop::execute_individual_agent(&app, &agent, force, config).await
            },
            StopTarget::Mcp { server_name, force } => {
                stop::execute_individual_mcp(&app, &server_name, force, config).await
            },
        };
    }

    let service_target = start::ServiceTarget::from_flags(flags);
    stop::execute(service_target, force, host).await
}

async fn execute_restart(
    target: Option<RestartTarget>,
    failed: bool,
    agents: bool,
    mcp: bool,
    host: &dyn KernelHost,
) -> Result<CommandOutput> {
    let config = host.output();
    if !(failed || agents || mcp) {
        return match target {
            Some(RestartTarget::Api) => restart::execute_api(host.prompter(), host).await,
            Some(RestartTarget::Agent { agent }) => {
                restart::execute_agent(&app_context(host).await?, &agent, config).await
            },
            Some(RestartTarget::Mcp { server_name, build }) => {
                restart::execute_mcp(&app_context(host).await?, &server_name, build, config).await
            },
            None => Err(anyhow::anyhow!(
                "Must specify target (api, agent, mcp) or use --failed/--agents/--mcp flag"
            )),
        };
    }
    let app = app_context(host).await?;
    if failed {
        restart::execute_failed(&app, config).await
    } else if agents {
        restart::execute_all_agents(&app, config).await
    } else {
        restart::execute_all_mcp(&app, config).await
    }
}

pub fn load_service_configs(
    boot: &systemprompt_loader::BootContext,
) -> Result<Vec<systemprompt_scheduler::ServiceConfig>> {
    let services_config = boot.load_services_config()?;
    Ok(systemprompt_scheduler::ServiceConfig::list_from_manifest(
        &services_config,
    ))
}
