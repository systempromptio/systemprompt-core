//! `plugins mcp tools` command collecting tools across running servers.
//!
//! The caller's session is resolved only when a queried server requires
//! OAuth; the command never starts a server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_identifiers::{McpServerId, SessionToken};
use systemprompt_manifest::ServicesConfig;
use systemprompt_mcp::McpServerConfig;

use super::call_client::{direct_url, list_tools};
use super::tools_schema::schema_view;
use super::types::{McpToolEntry, McpToolsOutput, McpToolsSummary};
use super::{observer, services_config};

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
    let services_config = services_config(host)
        .await
        .context("Failed to load services configuration")?;

    let running_servers = observer(host)
        .await?
        .running_servers()
        .await
        .context("Failed to get running servers")?;

    let servers_to_query = select_servers(&running_servers, args.server.as_ref())?;

    let token = if servers_to_query
        .iter()
        .any(|server| requires_auth(&services_config, server))
    {
        Some(host.caller_session().await?.token().clone())
    } else {
        None
    };

    let (mut all_tools, servers_queried) =
        collect_tools(&servers_to_query, &services_config, token.as_ref(), &args).await;

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

fn requires_auth(services_config: &ServicesConfig, server: &McpServerConfig) -> bool {
    services_config
        .mcp_servers
        .get(&server.name)
        .is_some_and(|c| c.oauth.required)
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
    services_config: &ServicesConfig,
    token: Option<&SessionToken>,
    args: &ToolsArgs,
) -> (Vec<McpToolEntry>, usize) {
    let mut all_tools = Vec::new();
    let mut servers_queried = 0;

    for server in servers {
        let server_token = token.filter(|_| requires_auth(services_config, server));
        let server_id = McpServerId::new(server.name.clone());
        let tools_result = match direct_url(server) {
            Ok(url) => list_tools(&server_id, &url, server_token, args.timeout).await,
            Err(e) => Err(e),
        };

        match tools_result {
            Ok(tools) => {
                all_tools.extend(tools.into_iter().map(|tool| {
                    McpToolEntry::from_probe(tool, server.name.clone(), args.detailed || args.schema)
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
