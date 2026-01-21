use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use rmcp::model::{
    CallToolRequestParam, ClientCapabilities, ClientInfo, Implementation, ProtocolVersion,
    RawContent,
};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::ServiceExt;
use std::time::Duration;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::client::HttpClientWithContext;
use systemprompt_mcp::services::McpManager;
use systemprompt_models::ai::tools::CallToolResult;
use systemprompt_runtime::AppContext;
use tokio::time::timeout;

use super::types::{McpCallOutput, McpToolContent};
use crate::session::{get_or_create_session, CliSessionContext};
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct CallArgs {
    #[arg(help = "MCP server name (required in non-interactive mode)")]
    pub server: Option<String>,

    #[arg(help = "Tool name to execute (required in non-interactive mode)")]
    pub tool: Option<String>,

    #[arg(short = 'a', long, help = "Tool arguments as JSON string")]
    pub args: Option<String>,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub async fn execute(args: CallArgs, config: &CliConfig) -> Result<CommandResult<McpCallOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let session_ctx = get_or_create_session(config).await?;

    let server_arg = args.server.clone();
    let tool_arg = args.tool.clone();
    let timeout_secs = args.timeout;

    let server_name = resolve_input(server_arg, "server", config, || {
        prompt_server_selection(&services_config)
    })?;

    let _server_config = services_config
        .mcp_servers
        .get(&server_name)
        .ok_or_else(|| anyhow!("MCP server '{}' not found in configuration", server_name))?;

    let ctx = AppContext::new()
        .await
        .context("Failed to initialize application context")?;

    let manager = McpManager::new(ctx.db_pool().clone()).context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    let running_server = running_servers
        .iter()
        .find(|s| s.name == server_name)
        .ok_or_else(|| anyhow!("MCP server '{}' is not running", server_name))?;

    let tool_name = resolve_input(tool_arg, "tool", config, || {
        prompt_tool_selection(
            &server_name,
            running_server.port,
            &session_ctx,
            timeout_secs,
        )
    })?;

    let tool_args: Option<serde_json::Value> = args
        .args
        .as_ref()
        .map(|s| serde_json::from_str(s))
        .transpose()
        .context("Invalid JSON in --args")?;

    let start_time = std::time::Instant::now();

    let result = execute_tool_call(
        &server_name,
        running_server.port,
        &tool_name,
        tool_args,
        &session_ctx,
        timeout_secs,
    )
    .await;

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let output = match result {
        Ok(tool_result) => {
            let content: Vec<McpToolContent> = tool_result
                .content
                .iter()
                .map(|c| convert_content(&c.raw))
                .collect();

            McpCallOutput {
                server: server_name.clone(),
                tool: tool_name.clone(),
                success: !tool_result.is_error.unwrap_or(false),
                content,
                execution_time_ms,
                error: None,
            }
        },
        Err(e) => McpCallOutput {
            server: server_name.clone(),
            tool: tool_name.clone(),
            success: false,
            content: vec![],
            execution_time_ms,
            error: Some(e.to_string()),
        },
    };

    Ok(CommandResult::card(output).with_title(format!("Tool Execution: {}", tool_name)))
}

fn convert_content(raw: &RawContent) -> McpToolContent {
    match raw {
        RawContent::Text(text) => McpToolContent {
            kind: "text".to_string(),
            text: Some(text.text.clone()),
            mime_type: None,
            data: None,
        },
        RawContent::Image(image) => McpToolContent {
            kind: "image".to_string(),
            text: None,
            mime_type: Some(image.mime_type.clone()),
            data: Some(image.data.clone()),
        },
        RawContent::Resource(resource) => McpToolContent {
            kind: "resource".to_string(),
            text: Some(format!("{:?}", resource.resource)),
            mime_type: None,
            data: None,
        },
        RawContent::Audio(audio) => McpToolContent {
            kind: "audio".to_string(),
            text: None,
            mime_type: Some(audio.mime_type.clone()),
            data: Some(audio.data.clone()),
        },
        RawContent::ResourceLink(link) => McpToolContent {
            kind: "resource_link".to_string(),
            text: Some(link.uri.clone()),
            mime_type: link.mime_type.clone(),
            data: None,
        },
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_tool_call(
    server_name: &str,
    port: u16,
    tool_name: &str,
    arguments: Option<serde_json::Value>,
    session_ctx: &CliSessionContext,
    timeout_secs: u64,
) -> Result<CallToolResult> {
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let request_context = session_ctx.to_request_context(&format!("cli-{}", server_name));
    let http_client = HttpClientWithContext::new(request_context);
    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", session_ctx.session_token().as_str()));
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

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

    let params = CallToolRequestParam {
        name: tool_name.to_string().into(),
        arguments: arguments.and_then(|v| v.as_object().cloned()),
    };

    let result = client
        .call_tool(params)
        .await
        .context("Tool execution failed")?;

    client.cancel().await?;
    Ok(result)
}

fn prompt_server_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut servers: Vec<&String> = config.mcp_servers.keys().collect();
    servers.sort();

    if servers.is_empty() {
        return Err(anyhow!("No MCP servers configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server")
        .items(&servers)
        .default(0)
        .interact()
        .context("Failed to get server selection")?;

    Ok(servers[selection].clone())
}

fn prompt_tool_selection(
    server_name: &str,
    port: u16,
    session_ctx: &CliSessionContext,
    timeout_secs: u64,
) -> Result<String> {
    let rt = tokio::runtime::Handle::current();
    let tools = rt.block_on(async {
        list_available_tools(server_name, port, session_ctx, timeout_secs).await
    })?;

    if tools.is_empty() {
        return Err(anyhow!("No tools available on server '{}'", server_name));
    }

    let tool_names: Vec<&str> = tools.iter().map(String::as_str).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select tool to execute")
        .items(&tool_names)
        .default(0)
        .interact()
        .context("Failed to get tool selection")?;

    Ok(tools[selection].clone())
}

async fn list_available_tools(
    server_name: &str,
    port: u16,
    session_ctx: &CliSessionContext,
    timeout_secs: u64,
) -> Result<Vec<String>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let request_context = session_ctx.to_request_context(&format!("cli-{}", server_name));
    let http_client = HttpClientWithContext::new(request_context);
    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", session_ctx.session_token().as_str()));
    let transport = StreamableHttpClientTransport::with_client(http_client, config);

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

    let tool_names: Vec<String> = tools_response
        .tools
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();

    client.cancel().await?;
    Ok(tool_names)
}
