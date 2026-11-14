//! `admin mcp`: hosting a YAML-declared MCP server in this binary.
//!
//! `admin mcp serve --server <id>` is what the API spawns for an internal MCP
//! server that lists `toolsets:` and names no `binary:`. It is a child host:
//! it boots a [`HostContext`] from the environment the API granted it (never
//! the CLI's profile, session or credentials) and serves the server's
//! toolsets on its `port:` until its supervisor stops it. The group registers
//! with the kernel through
//! [`register_cli_command!`](crate::register_cli_command).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::toolsets::serve;

use crate::kernel::{
    CliCommandGroup, CommandContext, CommandDescriptor, DataImpact, DescribeCommand,
};

#[derive(Debug, Clone, Subcommand)]
pub enum McpHostCommands {
    #[command(about = "Serve one YAML-declared MCP server's toolsets (spawned by the API)")]
    Serve(ServeArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ServeArgs {
    #[arg(
        long,
        help = "MCP server id, as declared under mcp_servers",
        value_parser = crate::shared::parse_mcp_server_id
    )]
    pub server: McpServerId,
}

impl DescribeCommand for McpHostCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Serve(_) => {
                CommandDescriptor::CHILD_HOST.with_data_impact(DataImpact::Preserving)
            },
        }
    }
}

impl CliCommandGroup for McpHostCommands {
    const PARENT: &'static [&'static str] = &["admin"];
    const NAME: &'static str = "mcp";
    const ABOUT: &'static str = "Host MCP servers in this binary";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = Result<()>> + Send {
        match self {
            Self::Serve(args) => execute_serve(args),
        }
    }
}

crate::register_cli_command!(McpHostCommands);

async fn execute_serve(args: ServeArgs) -> Result<()> {
    let host = HostContext::from_process_env()
        .await
        .context("Failed to boot the MCP host context")?;
    Box::pin(serve(Arc::new(host), args.server.clone()))
        .await
        .with_context(|| format!("MCP server '{}' stopped", args.server))
}
