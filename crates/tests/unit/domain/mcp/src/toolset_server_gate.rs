use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::host::ToolsetServer;
use systemprompt_mcp::middleware::{HopVerifier, admit_request};
use systemprompt_test_fixtures::{FIXTURE_EXTERNAL_URL, test_token_authority};

#[derive(Clone)]
struct Gate {
    server: ToolsetServer,
    server_id: McpServerId,
    headers: Vec<(String, String)>,
}

impl Gate {
    fn inject(&self, context: &mut RequestContext<RoleServer>) {
        let mut builder = http::Request::builder()
            .uri("http://localhost/mcp")
            .header("x-session-id", "s-host")
            .header("x-trace-id", "t-host")
            .header("x-agent-name", "agent-host");
        for (name, value) in &self.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let (mut parts, ()) = builder.body(()).expect("request").into_parts();
        let verifier = HopVerifier::new(
            test_token_authority(FIXTURE_EXTERNAL_URL),
            self.server_id.clone(),
        );
        admit_request(&verifier, &parts.headers, &mut parts.extensions).expect("admitted");
        context.extensions.insert(parts);
    }
}

impl ServerHandler for Gate {
    fn get_info(&self) -> ServerConfig {
        self.server.get_info()
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        mut context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.inject(&mut context);
        self.server.list_tools(request, context).await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        mut context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        self.inject(&mut context);
        self.server.call_tool(request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        mut context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.inject(&mut context);
        self.server.list_resources(request, context).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        mut context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        self.inject(&mut context);
        self.server.read_resource(request, context).await
    }
}

pub(crate) async fn connect(
    server: &ToolsetServer,
    headers: Vec<(String, String)>,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(16384);
    let gate = Gate {
        server: server.clone(),
        server_id: McpServerId::new(crate::toolset_server_live::SERVER),
        headers,
    };
    tokio::spawn(async move {
        if let Ok(running) = gate.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    ().serve(client_io).await.expect("client handshake")
}
