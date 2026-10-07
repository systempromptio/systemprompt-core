//! `infra services` command group: start, stop, restart, status, cleanup, and
//! serve.
//!
//! Defines the [`ServicesCommands`] clap subcommand tree and the individual
//! target enums ([`StartTarget`], [`StopTarget`], [`RestartTarget`]); dispatch
//! is delegated to the sibling implementation modules via [`execute`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cleanup;
mod dispatch;
mod lifecycle;
pub mod restart;
pub mod serve;
pub mod start;
pub mod status;
mod stop;
mod types;

use clap::Subcommand;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::McpServerId;

pub use dispatch::{execute, load_service_configs};

const DEFAULT_API_PORT: u16 = 8080;

pub(super) fn get_api_port() -> u16 {
    ProfileBootstrap::get().map_or(DEFAULT_API_PORT, |p| p.server.port)
}

pub(super) fn get_api_addr() -> anyhow::Result<String> {
    let profile = ProfileBootstrap::get()?;
    Ok(format!("{}:{}", profile.server.host, profile.server.port))
}

#[derive(Debug, Clone, Subcommand)]
pub enum StartTarget {
    #[command(about = "Start a single agent by name")]
    Agent { agent: String },
    #[command(about = "Start a single MCP server by name")]
    Mcp {
        #[arg(value_parser = crate::shared::parse_mcp_server_id)]
        server_name: McpServerId,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum StopTarget {
    #[command(about = "Stop a single agent by name")]
    Agent {
        agent: String,
        #[arg(long, help = "Force stop (SIGKILL)")]
        force: bool,
    },
    #[command(about = "Stop a single MCP server by name")]
    Mcp {
        #[arg(value_parser = crate::shared::parse_mcp_server_id)]
        server_name: McpServerId,
        #[arg(long, help = "Force stop (SIGKILL)")]
        force: bool,
    },
}

const KILL_PORT_PROCESS_HELP: &str = "Signal the process holding the API port even when it is \
                                      not a verified systemprompt API server (asks for \
                                      confirmation naming the PID; refused when non-interactive)";

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

        #[arg(long, help = KILL_PORT_PROCESS_HELP)]
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

        #[arg(long, help = KILL_PORT_PROCESS_HELP)]
        kill_port_process: bool,

        #[arg(
            long,
            help = "Skip database migrations (the schema is still verified to be current)"
        )]
        skip_migrate: bool,
    },
}

impl ServicesCommands {
    #[must_use]
    pub const fn serves_api(&self) -> bool {
        match self {
            Self::Serve { .. }
            | Self::Restart {
                target: Some(RestartTarget::Api),
                ..
            } => true,
            Self::Start {
                target: None,
                all,
                api,
                agents,
                mcp,
                ..
            } => *all || *api || !(*agents || *mcp),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum RestartTarget {
    #[command(about = "Restart the API server")]
    Api,
    #[command(about = "Restart a single agent by name")]
    Agent { agent: String },
    #[command(about = "Restart a single MCP server by name")]
    Mcp {
        #[arg(value_parser = crate::shared::parse_mcp_server_id)]
        server_name: McpServerId,
        #[arg(long, help = "Rebuild the binary before restarting")]
        build: bool,
    },
}
