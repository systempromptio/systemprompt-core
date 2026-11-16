//! `admin mcp`: hosting a YAML-declared MCP server in this binary.
//!
//! `admin mcp serve --server <id>` is what the API spawns for an internal MCP
//! server that lists `toolsets:` and names no `binary:`. It is a child host:
//! it boots a [`HostContext`] from the environment the API granted it (never
//! the CLI's profile, session or credentials) and serves the server's
//! toolsets on its `port:` until its supervisor stops it. It is a kernel
//! `admin` command with the child-host descriptor, and admits the composition
//! against the profile the API granted it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp_host::HostContext;
use systemprompt_mcp_host::toolsets::serve;

use systemprompt_extension::cli::{CommandDescriptor, CommandOutput, DataImpact, DescribeCommand};

use crate::kernel::KernelHost;

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
        value_parser = systemprompt_extension::cli::parsers::parse_mcp_server_id
    )]
    pub server: McpServerId,
}

impl DescribeCommand for McpHostCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Serve(_) => CommandDescriptor::NONE.with_data_impact(DataImpact::Preserving),
        }
    }
}

pub async fn execute(cmd: McpHostCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        McpHostCommands::Serve(args) => execute_serve(args, host).await?,
    }
    Ok(CommandOutput::rendered())
}

async fn execute_serve(args: ServeArgs, kernel: &dyn KernelHost) -> Result<()> {
    let host = HostContext::from_process_env(kernel.take_admit()?)
        .await
        .context("Failed to boot the MCP host context")?;
    Box::pin(serve(Arc::new(host), args.server.clone()))
        .await
        .with_context(|| format!("MCP server '{}' stopped", args.server))
}
