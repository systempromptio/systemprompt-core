//! Tool-call execution against a connected MCP server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::elicitation::SharedElicitationDelegate;
use super::http_client_with_context::HttpClientWithContext;
use super::{McpClientHandler, capabilities, tasks};
use crate::error::McpDomainResult;
use rmcp::model::{ClientInfo, Implementation, ProtocolVersion};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransport;
use rmcp::{RoleClient, ServiceExt};
use systemprompt_models::net::{HTTP_STREAM_CONNECT_TIMEOUT, MCP_TOOL_EXECUTION_TIMEOUT};
use tokio::time::timeout;


pub async fn execute_tool_call(
    transport: StreamableHttpClientTransport<HttpClientWithContext>,
    server: &str,
    name: &str,
    arguments: Option<serde_json::Value>,
    elicitation: Option<SharedElicitationDelegate>,
) -> McpDomainResult<systemprompt_models::CallToolResult> {
    // Why: rmcp's default is `ProtocolVersion::LATEST`, which is 2025-11-25 —
    // BELOW the 2026-07-28 that MRTR requires. A server refuses to hand an
    // `InputRequiredResult` to a peer that negotiated lower, so with the
    // default every `input_required` round became an error and no tool could
    // ever ask a human for anything through this client. Servers that do not
    // speak 2026-07-28 negotiate down as usual.
    let client_info = ClientInfo::new(
        capabilities::client_capabilities(elicitation.is_some()),
        Implementation::new("systemprompt-ai-mcp-client", "1.0.0"),
    )
    .with_protocol_version(ProtocolVersion::V_2026_07_28);

    let mut handler = McpClientHandler::new(client_info);
    if let Some(delegate) = elicitation {
        handler = handler.with_elicitation(delegate);
    }

    let client_service = match timeout(HTTP_STREAM_CONNECT_TIMEOUT, handler.serve(transport)).await
    {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            return Err(crate::error::McpDomainError::Timeout {
                server: server.to_owned(),
                after_ms: u64::try_from(HTTP_STREAM_CONNECT_TIMEOUT.as_millis())
                    .unwrap_or(u64::MAX),
            });
        },
    };

    let mut params = rmcp::model::CallToolRequestParams::new(name.to_owned());
    if let Some(args) = arguments.and_then(|v| v.as_object().cloned()) {
        params = params.with_arguments(args);
    }

    let result = timeout(
        MCP_TOOL_EXECUTION_TIMEOUT,
        dispatch_tool_call(&client_service, server, params),
    )
    .await
    .unwrap_or_else(|_| {
        Err(crate::error::McpDomainError::Timeout {
            server: server.to_owned(),
            after_ms: u64::try_from(MCP_TOOL_EXECUTION_TIMEOUT.as_millis()).unwrap_or(u64::MAX),
        })
    });

    client_service.cancel().await?;
    result
}

async fn dispatch_tool_call<S>(
    client: &rmcp::service::RunningService<RoleClient, S>,
    server: &str,
    params: rmcp::model::CallToolRequestParams,
) -> McpDomainResult<systemprompt_models::CallToolResult>
where
    S: rmcp::Service<RoleClient>,
{
    use rmcp::model::CallToolResponse;

    match client.call_tool_once(params.clone()).await.map_err(|e| {
        crate::error::McpDomainError::ToolExecutionFailed(format!("MCP tool call failed: {e}"))
    })? {
        CallToolResponse::Complete(result) => Ok(result),
        CallToolResponse::Task(created) => {
            tasks::poll_task_to_completion(client, server, created).await
        },
        // Why: rmcp's MRTR driver owns the input_required loop, but its retry
        // assembly is private, so the round is re-entered through `call_tool`;
        // SEP-2322 rounds are stateless on the server, so the extra initial
        // round-trip is harmless.
        CallToolResponse::InputRequired(_) => client.call_tool(params).await.map_err(|e| {
            crate::error::McpDomainError::ToolExecutionFailed(format!("MCP tool call failed: {e}"))
        }),
        other => Err(crate::error::McpDomainError::ToolExecutionFailed(format!(
            "unexpected tools/call response variant: {other:?}"
        ))),
    }
}
