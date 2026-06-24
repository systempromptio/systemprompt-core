use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::time::Duration;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, SessionToken, TraceId};
use systemprompt_mcp::services::client::HttpClientWithContext;
use systemprompt_models::execution::context::RequestContext;
use tokio::time::timeout;
use tracing::debug;

fn probe_context(server_name: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(format!("cli-{server_name}")),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
    )
}

pub(super) struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub parameters_count: usize,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

pub(super) async fn list_tools_unauthenticated(
    server_name: &str,
    port: u16,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
    let transport = StreamableHttpClientTransport::with_client(
        HttpClientWithContext::new(probe_context(server_name)),
        config,
    );

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{}", server_name), "1.0.0"),
    );

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
        .map(convert_tool_info)
        .collect();

    client.cancel().await?;
    Ok(tools)
}

pub(super) async fn list_tools_authenticated(
    server_name: &str,
    port: u16,
    token: &SessionToken,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);

    let config = StreamableHttpClientTransportConfig::with_uri(url.as_str())
        .auth_header(format!("Bearer {}", token.as_str()));
    let context = probe_context(server_name).with_auth_token(token.as_str());
    let transport =
        StreamableHttpClientTransport::with_client(HttpClientWithContext::new(context), config);

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new(format!("systemprompt-cli-{}", server_name), "1.0.0"),
    );

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
        .map(convert_tool_info)
        .collect();

    client.cancel().await?;
    Ok(tools)
}

fn convert_tool_info(tool: rmcp::model::Tool) -> ToolInfo {
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
}
