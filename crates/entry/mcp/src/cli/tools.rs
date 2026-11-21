//! `plugins mcp tools` command collecting tools across running servers.
//!
//! The caller's session is resolved only when a queried server is hosted by
//! the platform, and is presented only to those servers; an external server
//! never receives it, and one that requires a credential is refused by name.
//! The command never starts a server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CallerSession, CommandHost, CommandOutput};
use systemprompt_mcp::McpServerConfig;

use super::call_client::{McpTarget, ServerCredential, list_tools, server_credential};
use super::observer;
use super::tools_schema::schema_view;
use super::types::{McpToolEntry, McpToolsOutput, McpToolsSummary};

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[arg(long, short, help = "Filter to a specific MCP server")]
    pub server: Option<String>,

    #[arg(long, help = "Show full input/output schemas in JSON output")]
    pub detailed: bool,

    #[arg(long, help = "Display parameter schemas in a readable format")]
    pub schema: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(args: ToolsArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let running_servers = observer(host)
        .await?
        .running_servers()
        .await
        .context("Failed to get running servers")?;

    let servers_to_query = select_servers(&running_servers, args.server.as_ref())?;
    if args.server.is_some() {
        for server in &servers_to_query {
            if let Err(refused) = server_credential(server) {
                return Err(refused.into());
            }
        }
    }

    let session = if servers_to_query.iter().any(|server| {
        matches!(
            server_credential(server),
            Ok(ServerCredential::CallerSession)
        )
    }) {
        Some(host.caller_session().await?)
    } else {
        None
    };

    let (mut all_tools, servers_queried) =
        collect_tools(&servers_to_query, session.as_ref(), &args).await;

    all_tools.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));

    if args.schema && !host.output().is_json_output() {
        return Ok(CommandOutput::text_titled(
            "MCP Tool Schemas",
            schema_view(&all_tools),
        ));
    }

    let output = McpToolsOutput {
        summary: McpToolsSummary {
            total_tools: all_tools.len(),
            servers_queried,
        },
        tools: all_tools,
    };

    Ok(CommandOutput::table_of(
        vec!["name", "server", "description", "parameters_count"],
        &output.tools,
    )
    .with_title("MCP Tools"))
}

fn select_servers<'a>(
    running_servers: &'a [McpServerConfig],
    filter: Option<&String>,
) -> Result<Vec<&'a McpServerConfig>> {
    let selected: Vec<_> = filter.map_or_else(
        || running_servers.iter().collect(),
        |name| running_servers.iter().filter(|s| &s.name == name).collect(),
    );

    if selected.is_empty() {
        let message = filter.map_or_else(
            || "No MCP servers are currently running".to_owned(),
            |name| format!("MCP server '{name}' is not running"),
        );
        return Err(anyhow!(message));
    }

    Ok(selected)
}

async fn collect_tools(
    servers: &[&McpServerConfig],
    session: Option<&CallerSession>,
    args: &ToolsArgs,
) -> (Vec<McpToolEntry>, usize) {
    let mut all_tools = Vec::new();
    let mut servers_queried = 0;

    for server in servers {
        let tools_result = match McpTarget::resolve(server, session) {
            Ok(target) => list_tools(&target, args.timeout).await,
            Err(e) => Err(e),
        };

        match tools_result {
            Ok(tools) => {
                all_tools.extend(tools.into_iter().map(|tool| {
                    McpToolEntry::from_probe(
                        tool,
                        server.name.clone(),
                        args.detailed || args.schema,
                    )
                }));
                servers_queried += 1;
            },
            Err(e) => {
                tracing::warn!(
                    server = %server.name,
                    error = %e,
                    "Failed to list tools from server"
                );
            },
        }
    }

    (all_tools, servers_queried)
}
