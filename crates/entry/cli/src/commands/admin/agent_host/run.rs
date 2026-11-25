//! `admin agent-host run` command hosting one agent process.
//!
//! A spawned agent boots the host context the API granted it (it never loads
//! the profile's secrets source) and serves through the `DynAgentServer` the
//! `agent-http` member provides sealed on that host. A host without the
//! `agent-http` member admitted refuses, naming it; a host whose agent server
//! lacks a collaborator (the `ai` member disabled) refuses to serve, naming
//! it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Args;
use systemprompt_identifiers::AgentName;
use systemprompt_mcp_host::HostContext;
use systemprompt_traits::DynAgentServer;

use crate::kernel::KernelHost;

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run", value_parser = systemprompt_extension::cli::parsers::parse_agent_name)]
    pub agent_name: AgentName,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub(super) async fn execute(args: RunArgs, host: &dyn KernelHost) -> Result<()> {
    let context = HostContext::from_process_env(host.take_admit()?)
        .await
        .context("Failed to boot the agent host")?;
    let server = Arc::clone(context.state().get::<DynAgentServer>().context(
        "The agent-http member is not admitted on this host; it provides the agent server",
    )?);
    server
        .serve(&args.agent_name, args.port)
        .await
        .context("Failed to run agent server")
}
