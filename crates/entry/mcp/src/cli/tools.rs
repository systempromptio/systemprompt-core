//! `plugins mcp tools` command collecting tools across running servers.
//!
//! Every server is queried through the API's MCP proxy with the caller's own
//! session ([`super::probe`]); the command never starts a server and never
//! connects to a server's own port. `--agent <name>` restricts the query to
//! the servers that agent's definition includes, and reports each included
//! server that is not running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{CallerSession, CommandHost, CommandOutput};
use systemprompt_mcp::McpServerConfig;

use super::probe::{McpTarget, list_tools};
use super::tools_schema::schema_view;
use super::types::{McpToolEntry, McpToolsOutput, McpToolsSummary, McpUnavailableServer};
use super::{observer, proxy_caller, services_config};

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[arg(long, short, help = "Filter to a specific MCP server")]
    pub server: Option<String>,

    #[arg(
        long,
        conflicts_with = "server",
        help = "Only the MCP servers this agent's definition includes"
    )]
    pub agent: Option<String>,

    #[arg(long, help = "Show full input/output schemas in JSON output")]
    pub detailed: bool,

    #[arg(long, help = "Display parameter schemas in a readable format")]
    pub schema: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

struct Collected {
    tools: Vec<McpToolEntry>,
    unavailable: Vec<McpUnavailableServer>,
}

struct Selection<'a> {
    servers: Vec<&'a McpServerConfig>,
    unavailable: Vec<McpUnavailableServer>,
}

pub(super) async fn execute(args: ToolsArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let running_servers = observer(host)
        .await?
        .running_servers()
        .await
        .context("Failed to get running servers")?;

    let selection = match args.agent.as_deref() {
        Some(agent) => {
            let included = agent_servers(host, agent).await?;
            select_for_agent(&running_servers, &included)
        },
        None => Selection {
            servers: select_servers(&running_servers, args.server.as_ref())?,
            unavailable: Vec::new(),
        },
    };

    let mut collected = Collected {
        tools: Vec::new(),
        unavailable: selection.unavailable,
    };
    let servers_queried = if selection.servers.is_empty() {
        0
    } else {
        let (api_url, session) = proxy_caller(host).await?;
        collect_tools(&selection.servers, &api_url, &session, &args, &mut collected).await
    };
    let Collected {
        tools: mut all_tools,
        unavailable,
    } = collected;

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
        agent: args.agent.clone(),
        unavailable_servers: unavailable,
    };

    let title = args
        .agent
        .as_ref()
        .map_or_else(|| "MCP Tools".to_owned(), |agent| format!("MCP Tools: {agent}"));
    Ok(CommandOutput::table_of(
        vec!["name", "server", "description", "parameters_count"],
        &output.tools,
    )
    .with_title(title))
}

async fn agent_servers(host: &dyn CommandHost, agent: &str) -> Result<Vec<String>> {
    let services = services_config(host)
        .await
        .context("Failed to load services configuration")?;
    let definition = services
        .agents
        .get(agent)
        .ok_or_else(|| anyhow!("Agent '{agent}' not found"))?;
    Ok(definition.metadata.mcp_servers.include.clone())
}

fn select_for_agent<'a>(
    running_servers: &'a [McpServerConfig],
    included: &[String],
) -> Selection<'a> {
    let mut servers = Vec::new();
    let mut unavailable = Vec::new();
    for name in included {
        match running_servers.iter().find(|server| &server.name == name) {
            Some(server) => servers.push(server),
            None => unavailable.push(McpUnavailableServer {
                name: name.clone(),
                reason: "not running".to_owned(),
            }),
        }
    }
    Selection {
        servers,
        unavailable,
    }
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
    api_url: &str,
    session: &CallerSession,
    args: &ToolsArgs,
    collected: &mut Collected,
) -> usize {
    let mut servers_queried = 0;
    for server in servers {
        let target = McpTarget::through_proxy(api_url, server.server_id(), session);
        match list_tools(&target, args.timeout).await {
            Ok(tools) => {
                collected.tools.extend(tools.into_iter().map(|tool| {
                    McpToolEntry::from_probe(
                        tool,
                        server.name.clone(),
                        args.detailed || args.schema,
                    )
                }));
                servers_queried += 1;
            },
            Err(error) => {
                tracing::warn!(server = %server.name, error = %error, "Failed to list tools from server");
                collected.unavailable.push(McpUnavailableServer {
                    name: server.name.clone(),
                    reason: error.to_string(),
                });
            },
        }
    }
    servers_queried
}
