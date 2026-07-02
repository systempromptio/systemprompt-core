//! In-process MCP server over real streamable HTTP, used to drive the live
//! `systemprompt_mcp` client transport end to end.
//!
//! Stands up an `rmcp` [`StreamableHttpService`] backed by a trivial
//! [`ServerHandler`] that advertises one tool and echoes its arguments, bound
//! to an ephemeral loopback port via `axum`. Tests point the MCP client at the
//! returned `http://127.0.0.1:<port>/mcp` URL so the full reqwest + SSE +
//! session handshake runs against a genuine server rather than a wire mock.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, Implementation, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub(crate) struct EchoMcpServer {
    tool_name: String,
}

impl EchoMcpServer {
    pub(crate) fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
        }
    }

    fn echo_tool(&self) -> Tool {
        let mut schema = serde_json::Map::new();
        schema.insert("type".to_owned(), serde_json::json!("object"));
        let mut props = serde_json::Map::new();
        props.insert(
            "message".to_owned(),
            serde_json::json!({ "type": "string" }),
        );
        schema.insert("properties".to_owned(), serde_json::Value::Object(props));
        Tool::new(
            self.tool_name.clone(),
            "Echoes the message argument back to the caller",
            Arc::new(schema),
        )
    }
}

impl ServerHandler for EchoMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::default())
            .with_server_info(Implementation::new("echo-mcp-test-server", "9.9.9"))
            .with_instructions("test echo server")
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![self.echo_tool()]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if request.name.as_ref() != self.tool_name {
            return Err(McpError::invalid_params(
                format!("unknown tool: {}", request.name),
                None,
            ));
        }

        let message = request
            .arguments
            .as_ref()
            .and_then(|args| args.get("message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned();

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "echo: {message}"
        ))]))
    }
}

#[derive(Debug)]
pub(crate) struct RunningMockServer {
    pub(crate) url: String,
    pub(crate) host: String,
    pub(crate) port: u16,
    handle: JoinHandle<()>,
}

impl Drop for RunningMockServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub(crate) async fn start_echo_mcp_server(tool_name: &str) -> RunningMockServer {
    let tool_name = tool_name.to_owned();

    let service = StreamableHttpService::new(
        move || Ok(EchoMcpServer::new(tool_name.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = axum::Router::new().route_service("/mcp", service);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral bind for mock MCP server");
    let addr = listener.local_addr().expect("local_addr");
    let port = addr.port();

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service()).await;
    });

    RunningMockServer {
        url: format!("http://127.0.0.1:{port}/mcp"),
        host: "127.0.0.1".to_owned(),
        port,
        handle,
    }
}
