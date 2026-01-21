use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, ProtocolVersion};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::ServiceExt;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::SessionToken;
use tokio::time::timeout;
use tracing::debug;

use super::types::{AgentToolsOutput, AgentToolsSummary, UnavailableServer};
use crate::commands::plugins::mcp::types::McpToolEntry;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpManager;
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

pub async fn execute(
    args: ToolsArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentToolsOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let name = if let Some(n) = args.name {
        n
    } else if config.interactive {
        prompt_agent_selection(&services_config)?
    } else {
        return Err(anyhow!("Agent name is required in non-interactive mode"));
    };

    let agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

    let configured_servers = &agent.metadata.mcp_servers;

    if configured_servers.is_empty() {
        let output = AgentToolsOutput {
            agent: name.clone(),
            tools: Vec::new(),
            summary: AgentToolsSummary {
                total_tools: 0,
                configured_servers: 0,
                available_servers: 0,
            },
            unavailable_servers: Vec::new(),
        };
        return Ok(CommandResult::card(output)
            .with_title(format!("Agent Tools: {} (no MCP servers configured)", name)));
    }

    let session_ctx = get_or_create_session(config).await?;
    let session_token = session_ctx.session_token();

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let manager = McpManager::new(Arc::clone(&ctx)).context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    let running_server_names: std::collections::HashSet<_> =
        running_servers.iter().map(|s| s.name.as_str()).collect();

    let mut all_tools = Vec::new();
    let mut unavailable_servers = Vec::new();
    let mut servers_queried = 0;

    for server_name in configured_servers {
        if !running_server_names.contains(server_name.as_str()) {
            unavailable_servers.push(UnavailableServer {
                name: server_name.clone(),
                reason: "not running".to_string(),
            });
            continue;
        }

        let Some(server) = running_servers.iter().find(|s| &s.name == server_name) else {
            continue;
        };

        let server_config = services_config.mcp_servers.get(server_name);
        let requires_auth = server_config.is_some_and(|c| c.oauth.required);

        let tools_result = if requires_auth {
            list_tools_authenticated(server_name, server.port, session_token, args.timeout).await
        } else {
            list_tools_unauthenticated(server_name, server.port, args.timeout).await
        };

        match tools_result {
            Ok(tools) => {
                for tool in tools {
                    all_tools.push(McpToolEntry {
                        name: tool.name,
                        server: server_name.clone(),
                        description: tool.description,
                        parameters_count: tool.parameters_count,
                        input_schema: args.detailed.then_some(tool.input_schema).flatten(),
                        output_schema: args.detailed.then_some(tool.output_schema).flatten(),
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
                unavailable_servers.push(UnavailableServer {
                    name: server_name.clone(),
                    reason: format!("connection failed: {}", e),
                });
            },
        }
    }

    all_tools.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));

    let output = AgentToolsOutput {
        agent: name.clone(),
        tools: all_tools.clone(),
        summary: AgentToolsSummary {
            total_tools: all_tools.len(),
            configured_servers: configured_servers.len(),
            available_servers: servers_queried,
        },
        unavailable_servers,
    };

    let columns = vec![
        "name".to_string(),
        "server".to_string(),
        "description".to_string(),
        "parameters_count".to_string(),
    ];

    Ok(CommandResult::table(output)
        .with_title(format!("Agent Tools: {}", name))
        .with_columns(columns))
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

struct ToolInfo {
    name: String,
    description: Option<String>,
    parameters_count: usize,
    input_schema: Option<serde_json::Value>,
    output_schema: Option<serde_json::Value>,
}

async fn list_tools_unauthenticated(
    server_name: &str,
    port: u16,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let transport = StreamableHttpClientTransport::from_uri(url.as_str());

    let client_info = ClientInfo {
        protocol_version: ProtocolVersion::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: format!("systemprompt-cli-{}", server_name),
            title: None,
            version: "1.0.0".to_string(),
            website_url: None,
            icons: None,
        },
    };

    let client = timeout(
        Duration::from_secs(timeout_secs),
        client_info.serve(transport),
    )
    .await
    .context("Connection timeout")?
    .context("Failed to connect to MCP server")?;

    let tools_response = client
        .list_tools(None)
        .await
        .context("Failed to list tools")?;

    let tools: Vec<ToolInfo> = tools_response
        .tools
        .into_iter()
        .map(|tool| {
            let input_schema = serde_json::to_value(&tool.input_schema)
                .inspect_err(|e| debug!("Failed to serialize input schema: {}", e))
                .ok();
            let output_schema = tool.output_schema.and_then(|s| {
                serde_json::to_value(s.as_ref())
                    .inspect_err(|e| debug!("Failed to serialize output schema: {}", e))
                    .ok()
            });
            let parameters_count = input_schema
                .as_ref()
                .and_then(|s| s.get("properties"))
                .and_then(|p| p.as_object())
                .map_or(0, serde_json::Map::len);

            ToolInfo {
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()),
                parameters_count,
                input_schema,
                output_schema,
            }
        })
        .collect();

    client.cancel().await?;
    Ok(tools)
}

async fn list_tools_authenticated(
    server_name: &str,
    port: u16,
    token: &SessionToken,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", token.as_str()));
    let transport = StreamableHttpClientTransport::from_config(config);

    let client_info = ClientInfo {
        protocol_version: ProtocolVersion::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: format!("systemprompt-cli-{}", server_name),
            title: None,
            version: "1.0.0".to_string(),
            website_url: None,
            icons: None,
        },
    };

    let client = timeout(
        Duration::from_secs(timeout_secs),
        client_info.serve(transport),
    )
    .await
    .context("Connection timeout")?
    .context("Failed to connect to MCP server")?;

    let tools_response = client
        .list_tools(None)
        .await
        .context("Failed to list tools")?;

    let tools: Vec<ToolInfo> = tools_response
        .tools
        .into_iter()
        .map(|tool| {
            let input_schema = serde_json::to_value(&tool.input_schema)
                .inspect_err(|e| debug!("Failed to serialize input schema: {}", e))
                .ok();
            let output_schema = tool.output_schema.and_then(|s| {
                serde_json::to_value(s.as_ref())
                    .inspect_err(|e| debug!("Failed to serialize output schema: {}", e))
                    .ok()
            });
            let parameters_count = input_schema
                .as_ref()
                .and_then(|s| s.get("properties"))
                .and_then(|p| p.as_object())
                .map_or(0, serde_json::Map::len);

            ToolInfo {
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()),
                parameters_count,
                input_schema,
                output_schema,
            }
        })
        .collect();

    client.cancel().await?;
    Ok(tools)
}
