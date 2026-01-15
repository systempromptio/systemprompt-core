use anyhow::{anyhow, Context, Result};
use clap::Args;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation, ProtocolVersion};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::ServiceExt;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::SessionToken;
use tokio::time::timeout;

use super::types::{McpToolEntry, McpToolsOutput, McpToolsSummary};
use crate::session::get_or_create_session;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[arg(long, short, help = "Filter to a specific MCP server")]
    pub server: Option<String>,

    #[arg(long, help = "Show full input/output schemas")]
    pub detailed: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub async fn execute(args: ToolsArgs, config: &CliConfig) -> Result<CommandResult<McpToolsOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

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

    let servers_to_query: Vec<_> = if let Some(ref filter) = args.server {
        running_servers
            .iter()
            .filter(|s| &s.name == filter)
            .collect()
    } else {
        running_servers.iter().collect()
    };

    if servers_to_query.is_empty() {
        let message = args.server.as_ref().map_or_else(
            || "No MCP servers are currently running".to_string(),
            |name| format!("MCP server '{}' is not running", name),
        );
        return Err(anyhow!(message));
    }

    let mut all_tools = Vec::new();
    let mut servers_queried = 0;

    for server in &servers_to_query {
        let server_config = services_config.mcp_servers.get(&server.name);
        let requires_auth = server_config.is_some_and(|c| c.oauth.required);

        let tools_result = if requires_auth {
            list_tools_authenticated(&server.name, server.port, session_token, args.timeout).await
        } else {
            list_tools_unauthenticated(&server.name, server.port, args.timeout).await
        };

        match tools_result {
            Ok(tools) => {
                for tool in tools {
                    all_tools.push(McpToolEntry {
                        name: tool.name,
                        server: server.name.clone(),
                        description: tool.description,
                        parameters_count: tool.parameters_count,
                        input_schema: args.detailed.then(|| tool.input_schema).flatten(),
                        output_schema: args.detailed.then(|| tool.output_schema).flatten(),
                    });
                }
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

    all_tools.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));

    let output = McpToolsOutput {
        tools: all_tools.clone(),
        summary: McpToolsSummary {
            total_tools: all_tools.len(),
            servers_queried,
        },
    };

    let columns = vec![
        "name".to_string(),
        "server".to_string(),
        "description".to_string(),
        "parameters_count".to_string(),
    ];

    Ok(CommandResult::table(output)
        .with_title("MCP Tools")
        .with_columns(columns))
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
            let input_schema = serde_json::to_value(&tool.input_schema).ok();
            let output_schema = tool
                .output_schema
                .and_then(|s| serde_json::to_value(s.as_ref()).ok());
            let parameters_count = input_schema
                .as_ref()
                .and_then(|s| s.get("properties"))
                .and_then(|p| p.as_object())
                .map_or(0, |o| o.len());

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
            let input_schema = serde_json::to_value(&tool.input_schema).ok();
            let output_schema = tool
                .output_schema
                .and_then(|s| serde_json::to_value(s.as_ref()).ok());
            let parameters_count = input_schema
                .as_ref()
                .and_then(|s| s.get("properties"))
                .and_then(|p| p.as_object())
                .map_or(0, |o| o.len());

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
