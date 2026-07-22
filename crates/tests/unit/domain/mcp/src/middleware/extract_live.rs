//! Exercises `extract_bearer_token` / `extract_request_context` with real
//! `RequestContext<RoleServer>` values by serving an in-process rmcp handler
//! over a duplex transport and probing both the missing-parts and
//! parts-present arms from inside `list_tools`.

use std::future::Future;
use std::sync::Arc;

use rmcp::model::{ListToolsResult, PaginatedRequestParams, Tool};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::middleware::{extract_bearer_token, extract_request_context};
use systemprompt_models::RequestContext as SysRequestContext;

fn sys_ctx() -> SysRequestContext {
    SysRequestContext::new(
        SessionId::new("s-live"),
        TraceId::new("t-live"),
        ContextId::generate(),
        AgentName::new("agent-live"),
    )
    .with_actor(Actor::user(UserId::new("user-live")))
}

fn tool(name: String) -> Tool {
    Tool::new(name, "probe outcome", Arc::new(serde_json::Map::new()))
}

#[derive(Clone)]
struct ExtractProbe;

impl ServerHandler for ExtractProbe {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        mut context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let mut outcomes = Vec::new();

        outcomes.push(format!(
            "no-parts-bearer:{}",
            match extract_bearer_token(&context) {
                Ok(_) => "ok".to_owned(),
                Err(err) => format!("err={}", err.message),
            }
        ));
        outcomes.push(format!(
            "no-parts-ctx:{}",
            match extract_request_context(&context) {
                Ok(_) => "ok".to_owned(),
                Err(err) => format!("err={}", err.message),
            }
        ));

        let mut builder = http::Request::builder()
            .uri("http://localhost/mcp")
            .header("authorization", "Bearer tok-live");
        builder = builder.header("content-type", "application/json");
        let (mut parts, ()) = builder.body(()).expect("request").into_parts();
        parts.extensions.insert(sys_ctx());
        context.extensions.insert(parts);

        outcomes.push(format!(
            "bearer:{}",
            extract_bearer_token(&context)
                .expect("parts present")
                .unwrap_or_default()
        ));
        let recovered = extract_request_context(&context).expect("context extension present");
        outcomes.push(format!("session:{}", recovered.session_id().as_str()));

        std::future::ready(Ok(ListToolsResult {
            tools: outcomes.into_iter().map(tool).collect(),
            next_cursor: None,
            meta: None,
        }))
    }
}

#[tokio::test]
async fn extract_helpers_observe_live_request_context() {
    let (server_io, client_io) = tokio::io::duplex(4096);

    let server = tokio::spawn(async move {
        ExtractProbe
            .serve(server_io)
            .await
            .expect("server handshake")
            .waiting()
            .await
    });

    let client = ().serve(client_io).await.expect("client handshake");
    let listed = client
        .list_tools(None)
        .await
        .expect("probe handler responds");

    let names: Vec<String> = listed
        .tools
        .iter()
        .map(|t| t.name.clone().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("no-parts-bearer:err=") && n.contains("No HTTP parts")),
        "got: {names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("no-parts-ctx:err=") && n.contains("RequestContext missing")),
        "got: {names:?}"
    );
    assert!(names.contains(&"bearer:tok-live".to_owned()), "{names:?}");
    assert!(names.contains(&"session:s-live".to_owned()), "{names:?}");

    client.cancel().await.expect("client shutdown");
    server.abort();
}
