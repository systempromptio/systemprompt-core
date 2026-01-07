mod cleanup;
pub mod db;
pub mod restart;
pub mod scheduler;
pub mod serve;
mod start;
mod status;
mod stop;

use anyhow::{Context, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

#[derive(Subcommand)]
pub enum ServicesCommands {
    #[command(about = "Start services")]
    Start {
        #[arg(long, help = "Start all services")]
        all: bool,

        #[arg(long, help = "Start API server only")]
        api: bool,

        #[arg(long, help = "Start agents only")]
        agents: bool,

        #[arg(long, help = "Start MCP servers only")]
        mcp: bool,

        #[arg(long, help = "Run in foreground (default)")]
        foreground: bool,

        #[arg(long, help = "Skip web asset build")]
        skip_web: bool,

        #[arg(long, help = "Skip database migrations")]
        skip_migrate: bool,
    },

    #[command(about = "Stop services")]
    Stop {
        #[arg(long, help = "Stop all services")]
        all: bool,

        #[arg(long, help = "Stop API server only")]
        api: bool,

        #[arg(long, help = "Stop agents only")]
        agents: bool,

        #[arg(long, help = "Stop MCP servers only")]
        mcp: bool,

        #[arg(long, help = "Force stop (SIGKILL)")]
        force: bool,
    },

    #[command(about = "Restart services")]
    Restart {
        #[command(subcommand)]
        target: Option<RestartTarget>,

        #[arg(long, help = "Restart only failed services")]
        failed: bool,
    },

    #[command(about = "Show detailed service status")]
    Status {
        #[arg(long, help = "Show detailed information")]
        detailed: bool,

        #[arg(long, help = "Output as JSON")]
        json: bool,

        #[arg(long, help = "Include health check results")]
        health: bool,
    },

    #[command(about = "Clean up orphaned processes and stale entries")]
    Cleanup,

    #[command(about = "Start API server (automatically starts agents and MCP servers)")]
    Serve {
        #[arg(long, help = "Run in foreground mode")]
        foreground: bool,
    },

    #[command(subcommand, about = "Database operations")]
    Db(db::DbCommands),

    #[command(subcommand, about = "Scheduler and background jobs")]
    Scheduler(scheduler::SchedulerCommands),
}

#[derive(Subcommand, Clone)]
pub enum RestartTarget {
    Api,
    Agent { agent_id: String },
    Mcp { server_name: String },
}

pub async fn execute(command: ServicesCommands) -> Result<()> {
    match command {
        ServicesCommands::Start {
            all,
            api,
            agents,
            mcp,
            foreground: _,
            skip_web,
            skip_migrate,
        } => {
            let flags = start::ServiceFlags {
                all,
                targets: start::ServiceTargetFlags { api, agents, mcp },
            };
            let target = start::ServiceTarget::from_flags(flags);
            let options = start::StartupOptions {
                skip_web,
                skip_migrate,
            };
            start::execute(target, options).await
        },

        ServicesCommands::Stop {
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
            let target = start::ServiceTarget::from_flags(flags);
            stop::execute(target, force).await
        },

        ServicesCommands::Restart { target, failed } => {
            let ctx = Arc::new(
                AppContext::new()
                    .await
                    .context("Failed to initialize application context")?,
            );

            if failed {
                restart::execute_failed(&ctx).await
            } else {
                match target {
                    Some(RestartTarget::Api) => {
                        restart::execute_api(&ctx);
                        Ok(())
                    },
                    Some(RestartTarget::Agent { agent_id }) => {
                        restart::execute_agent(&ctx, &agent_id).await
                    },
                    Some(RestartTarget::Mcp { server_name }) => {
                        restart::execute_mcp(&ctx, &server_name).await
                    },
                    None => Err(anyhow::anyhow!(
                        "Must specify target (api, agent, mcp) or use --failed flag"
                    )),
                }
            }
        },

        ServicesCommands::Status {
            detailed,
            json,
            health,
        } => status::execute(detailed, json, health).await,

        ServicesCommands::Cleanup => cleanup::execute().await,

        ServicesCommands::Serve { foreground } => serve::execute(foreground).await,

        ServicesCommands::Db(cmd) => db::execute(cmd).await,

        ServicesCommands::Scheduler(cmd) => {
            let ctx = Arc::new(
                AppContext::new()
                    .await
                    .context("Failed to initialize application context")?,
            );
            scheduler::execute(cmd, ctx).await
        },
    }
}

pub fn load_service_configs(
    _ctx: &AppContext,
) -> Result<Vec<systemprompt_core_scheduler::ServiceConfig>> {
    use systemprompt_core_scheduler::{ServiceConfig, ServiceType};
    use systemprompt_loader::ConfigLoader;

    let services_config = ConfigLoader::load()?;
    let mut configs = Vec::new();

    for (name, agent) in &services_config.agents {
        configs.push(ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Agent,
            port: agent.port,
            enabled: agent.enabled,
        });
    }

    for (name, mcp) in &services_config.mcp_servers {
        configs.push(ServiceConfig {
            name: name.clone(),
            service_type: ServiceType::Mcp,
            port: mcp.port,
            enabled: mcp.enabled,
        });
    }

    Ok(configs)
}
