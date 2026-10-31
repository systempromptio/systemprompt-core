use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::host::ToolsetServer;
use systemprompt_models::RequestContext as SysRequestContext;

#[derive(Clone)]
struct Gate {
    server: ToolsetServer,
    bearer: Option<String>,
}

impl Gate {
    fn inject(&self, context: &mut RequestContext<RoleServer>) {
        let mut builder = http::Request::builder().uri("http://localhost/mcp");
        if let Some(bearer) = &self.bearer {
            builder = builder.header("authorization", bearer.as_str());
        }
        let (mut parts, ()) = builder.body(()).expect("request").into_parts();
        parts.extensions.insert(SysRequestContext::new(
            SessionId::new("s-host"),
            TraceId::new("t-host"),
            ContextId::generate(),
            AgentName::try_new("agent-host").expect("agent"),
            Actor::user(UserId::new("user-host")),
        ));
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
    bearer: Option<String>,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(16384);
    let gate = Gate {
        server: server.clone(),
        bearer,
    };
    tokio::spawn(async move {
        if let Ok(running) = gate.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    ().serve(client_io).await.expect("client handshake")
}
