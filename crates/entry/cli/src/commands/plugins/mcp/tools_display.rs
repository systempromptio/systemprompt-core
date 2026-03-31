use anyhow::{Context, Result};
use rmcp::ServiceExt;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::time::Duration;
use systemprompt_identifiers::SessionToken;
use systemprompt_logging::CliService;
use tokio::time::timeout;
use tracing::debug;

use super::types::McpToolEntry;

pub(super) struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub parameters_count: usize,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
}

pub(super) fn print_schema_view(tools: &[McpToolEntry]) {
    CliService::section("MCP Tool Schemas");

    for tool in tools {
        CliService::info("");
        CliService::info(&format!("╭─ {}/{}", tool.server, tool.name));

        if let Some(ref desc) = tool.description {
            CliService::info(&format!("│  {}", desc));
        }

        if let Some(ref schema) = tool.input_schema {
            CliService::info("│");
            CliService::info("│  Parameters:");
            print_schema_properties(schema, "│    ");
        } else {
            CliService::info("│  (no parameters)");
        }

        CliService::info("╰─");
    }
}

fn print_schema_properties(schema: &serde_json::Value, indent: &str) {
    let properties = schema.get("properties").and_then(|p| p.as_object());
    let required = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map_or_else(std::collections::HashSet::new, |arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<std::collections::HashSet<_>>()
        });

    if let Some(props) = properties {
        for (name, prop_schema) in props {
            let is_required = required.contains(name.as_str());
            let req_marker = if is_required { "*" } else { "" };

            let prop_type = prop_schema
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("any");

            let description = prop_schema
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");

            let type_display = prop_schema
                .get("enum")
                .and_then(|e| e.as_array())
                .map_or_else(
                    || prop_type.to_string(),
                    |values| {
                        let vals: Vec<String> = values
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| format!("\"{}\"", s)))
                            .collect();
                        format!("enum[{}]", vals.join("|"))
                    },
                );

            CliService::info(&format!(
                "{}{}{}: {} - {}",
                indent, name, req_marker, type_display, description
            ));
        }
    }
}

fn extract_tools(tools_response: rmcp::model::ListToolsResult) -> Vec<ToolInfo> {
    tools_response
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
        .collect()
}

pub(super) async fn list_tools_unauthenticated(
    server_name: &str,
    port: u16,
    timeout_secs: u64,
) -> Result<Vec<ToolInfo>> {
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let transport = StreamableHttpClientTransport::from_uri(url.as_str());

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

    let tools = extract_tools(tools_response);
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
    let transport = StreamableHttpClientTransport::from_config(config);

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

    let tools = extract_tools(tools_response);
    client.cancel().await?;
    Ok(tools)
}
