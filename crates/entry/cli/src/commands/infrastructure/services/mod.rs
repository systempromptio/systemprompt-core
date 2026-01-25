mod cleanup;
pub mod restart;
pub mod serve;
mod start;
mod status;
mod stop;

use crate::cli_settings::CliConfig;
use anyhow::{Context, Result};
use clap::Subcommand;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Subcommand)]
pub enum StartTarget {
    Agent { agent_id: String },
    Mcp { server_name: String },
}

#[derive(Debug, Clone, Subcommand)]
pub enum StopTarget {
    Agent {
        agent_id: String,
        #[arg(long, help = "Force stop (SIGKILL)")]
        force: bool,
    },
    Mcp {
        server_name: String,
        #[arg(long, help = "Force stop (SIGKILL)")]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ServicesCommands {
    #[command(
        about = "Start API, agents, and MCP servers",
        after_help = "EXAMPLES:\n  systemprompt infra services start\n  systemprompt infra \
                      services start --api\n  systemprompt infra services start --agents --mcp\n  \
                      systemprompt infra services start agent <name>"
    )]
    Start {
        #[command(subcommand)]
        target: Option<StartTarget>,

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

        #[arg(long, help = "Skip database migrations")]
        skip_migrate: bool,

        #[arg(long, help = "Kill process using the port if occupied")]
        kill_port_process: bool,
    },

    #[command(
        about = "Stop running services gracefully",
        after_help = "EXAMPLES:\n  systemprompt infra services stop\n  systemprompt infra \
                      services stop --api\n  systemprompt infra services stop agent <name> \
                      [--force]"
    )]
    Stop {
        #[command(subcommand)]
        target: Option<StopTarget>,

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

        #[arg(long, help = "Restart all agents")]
        agents: bool,

        #[arg(long, help = "Restart all MCP servers")]
        mcp: bool,
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
    Cleanup {
        #[arg(short = 'y', long, help = "Skip confirmation prompt")]
        yes: bool,

        #[arg(long, help = "Preview cleanup without executing")]
        dry_run: bool,
    },

    #[command(about = "Start API server (automatically starts agents and MCP servers)")]
    Serve {
        #[arg(long, help = "Run in foreground mode")]
        foreground: bool,

        #[arg(long, help = "Kill process using the port if occupied")]
        kill_port_process: bool,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum RestartTarget {
    Api,
    Agent {
        agent_id: String,
    },
    Mcp {
        server_name: String,
        #[arg(long, help = "Rebuild the binary before restarting")]
        build: bool,
    },
}

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
            if let Some(individual) = target {
                let ctx = Arc::new(
                    AppContext::new()
                        .await
                        .context("Failed to initialize application context")?,
                );
                return match individual {
                    StartTarget::Agent { agent_id } => {
                        start::execute_individual_agent(&ctx, &agent_id, config).await
                    },
                    StartTarget::Mcp { server_name } => {
                        start::execute_individual_mcp(&ctx, &server_name, config).await
                    },
                };
            }

            let flags = start::ServiceFlags {
                all,
                targets: start::ServiceTargetFlags { api, agents, mcp },
            };
            let service_target = start::ServiceTarget::from_flags(flags);
            let options = start::StartupOptions {
                skip_migrate,
                kill_port_process,
            };
            start::execute(service_target, options, config).await
        },

        ServicesCommands::Stop {
            target,
            all,
            api,
            agents,
            mcp,
            force,
        } => {
            if let Some(individual) = target {
                let ctx = Arc::new(
                    AppContext::new()
                        .await
                        .context("Failed to initialize application context")?,
                );
                return match individual {
                    StopTarget::Agent { agent_id, force } => {
                        stop::execute_individual_agent(&ctx, &agent_id, force, config).await
                    },
                    StopTarget::Mcp { server_name, force } => {
                        stop::execute_individual_mcp(&ctx, &server_name, force, config).await
                    },
                };
            }

            let flags = start::ServiceFlags {
                all,
                targets: start::ServiceTargetFlags { api, agents, mcp },
            };
            let service_target = start::ServiceTarget::from_flags(flags);
            stop::execute(service_target, force, config).await
        },

        ServicesCommands::Restart {
            target,
            failed,
            agents,
            mcp,
        } => {
            let ctx = Arc::new(
                AppContext::new()
                    .await
                    .context("Failed to initialize application context")?,
            );

            if failed {
                restart::execute_failed(&ctx, config).await
            } else if agents {
                restart::execute_all_agents(&ctx, config).await
            } else if mcp {
                restart::execute_all_mcp(&ctx, config).await
            } else {
                match target {
                    Some(RestartTarget::Api) => restart::execute_api(config).await,
                    Some(RestartTarget::Agent { agent_id }) => {
                        restart::execute_agent(&ctx, &agent_id, config).await
                    },
                    Some(RestartTarget::Mcp { server_name, build }) => {
                        restart::execute_mcp(&ctx, &server_name, build, config).await
                    },
                    None => Err(anyhow::anyhow!(
                        "Must specify target (api, agent, mcp) or use --failed/--agents/--mcp flag"
                    )),
                }
            }
        },

        ServicesCommands::Status {
            detailed,
            json,
            health,
        } => status::execute(detailed, json, health, config).await,

        ServicesCommands::Cleanup { yes, dry_run } => cleanup::execute(yes, dry_run, config).await,

        ServicesCommands::Serve {
            foreground,
            kill_port_process,
        } => serve::execute(foreground, kill_port_process, config).await,
    }
}

pub fn load_service_configs(
    _ctx: &AppContext,
) -> Result<Vec<systemprompt_scheduler::ServiceConfig>> {
    use systemprompt_loader::ConfigLoader;
    use systemprompt_scheduler::{ServiceConfig, ServiceType};

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
