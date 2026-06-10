use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use super::tools_mcp::{list_tools_authenticated, list_tools_unauthenticated};
use super::types::{AgentToolsOutput, AgentToolsSummary, UnavailableServer};
use crate::CliConfig;
use crate::commands::plugins::mcp::types::McpToolEntry;
use crate::context::CommandContext;
use crate::session::get_or_create_session;
use crate::shared::CommandOutput;
use systemprompt_identifiers::SessionToken;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::McpServerConfig;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(long, help = "Show full input/output schemas")]
    pub detailed: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds per server")]
    pub timeout: u64,
}

struct ToolQuery<'a> {
    services_config: &'a systemprompt_models::ServicesConfig,
    running_servers: &'a [McpServerConfig],
    session_token: &'a SessionToken,
    detailed: bool,
    timeout: u64,
}

struct CollectedTools {
    tools: Vec<McpToolEntry>,
    unavailable: Vec<UnavailableServer>,
    servers_queried: usize,
}

pub(super) async fn execute(args: ToolsArgs, ctx: &CommandContext) -> Result<CommandOutput> {
    let config = &ctx.cli;
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let name = resolve_agent_name(args.name, config, &services_config)?;

    let agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    let configured_servers = &agent.metadata.mcp_servers.include;

    if configured_servers.is_empty() {
        return Ok(no_servers_output(&name));
    }

    let session_ctx = get_or_create_session(ctx).await?;

    let ctx = AppContext::new()
        .await
        .context("Failed to initialize application context")?;

    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )
    .context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    let query = ToolQuery {
        services_config: &services_config,
        running_servers: &running_servers,
        session_token: session_ctx.session_token(),
        detailed: args.detailed,
        timeout: args.timeout,
    };
    let mut collected = collect_tools(configured_servers, &query).await;
    collected
        .tools
        .sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));

    let output = AgentToolsOutput {
        agent: name.clone(),
        tools: collected.tools.clone(),
        summary: AgentToolsSummary {
            total_tools: collected.tools.len(),
            configured_servers: configured_servers.len(),
            available_servers: collected.servers_queried,
        },
        unavailable_servers: collected.unavailable,
    };

    let columns = vec![
        "name".to_owned(),
        "server".to_owned(),
        "description".to_owned(),
        "parameters_count".to_owned(),
    ];

    Ok(
        CommandOutput::table_of(columns, &output.tools)
            .with_title(format!("Agent Tools: {}", name)),
    )
}

fn resolve_agent_name(
    name: Option<String>,
    config: &CliConfig,
    services_config: &systemprompt_models::ServicesConfig,
) -> Result<String> {
    match name {
        Some(n) => Ok(n),
        None if config.interactive => prompt_agent_selection(services_config),
        None => Err(anyhow!("Agent name is required in non-interactive mode")),
    }
}

fn no_servers_output(name: &str) -> CommandOutput {
    let output = AgentToolsOutput {
        agent: name.to_owned(),
        tools: Vec::new(),
        summary: AgentToolsSummary {
            total_tools: 0,
            configured_servers: 0,
            available_servers: 0,
        },
        unavailable_servers: Vec::new(),
    };
    CommandOutput::card_value(
        format!("Agent Tools: {} (no MCP servers configured)", name),
        &output,
    )
}

async fn collect_tools(configured_servers: &[String], query: &ToolQuery<'_>) -> CollectedTools {
    let running_server_names: std::collections::HashSet<_> = query
        .running_servers
        .iter()
        .map(|s| s.name.as_str())
        .collect();

    let mut tools = Vec::new();
    let mut unavailable = Vec::new();
    let mut servers_queried = 0;

    for server_name in configured_servers {
        if !running_server_names.contains(server_name.as_str()) {
            unavailable.push(UnavailableServer {
                name: server_name.clone(),
                reason: "not running".to_owned(),
            });
            continue;
        }

        let Some(server) = query
            .running_servers
            .iter()
            .find(|s| &s.name == server_name)
        else {
            continue;
        };

        let server_config = query.services_config.mcp_servers.get(server_name);
        let requires_auth = server_config.is_some_and(|c| c.oauth.required);

        let tools_result = if requires_auth {
            list_tools_authenticated(server_name, server.port, query.session_token, query.timeout)
                .await
        } else {
            list_tools_unauthenticated(server_name, server.port, query.timeout).await
        };

        match tools_result {
            Ok(server_tools) => {
                for tool in server_tools {
                    tools.push(McpToolEntry {
                        name: tool.name,
                        server: server_name.clone(),
                        description: tool.description,
                        parameters_count: tool.parameters_count,
                        input_schema: query.detailed.then_some(tool.input_schema).flatten(),
                        output_schema: query.detailed.then_some(tool.output_schema).flatten(),
                    });
                }
                servers_queried += 1;
            },
            Err(e) => {
                tracing::warn!(
                    server = %server_name,
                    error = %e,
                    "Failed to list tools from server"
                );
                unavailable.push(UnavailableServer {
                    name: server_name.clone(),
                    reason: format!("connection failed: {}", e),
                });
            },
        }
    }

    CollectedTools {
        tools,
        unavailable,
        servers_queried,
    }
}

fn prompt_agent_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut agents: Vec<&String> = config.agents.keys().collect();
    agents.sort();

    if agents.is_empty() {
        return Err(anyhow!("No agents configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
