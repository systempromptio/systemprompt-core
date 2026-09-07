use serde_json::{Value, json};
use systemprompt_cli::admin::agents::test_api as agent;
use systemprompt_cli::plugins::mcp::test_api as plugin;
use systemprompt_identifiers::SessionToken;
use wiremock::matchers::{body_partial_json, method};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

async fn server(fail_list: bool) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(body_partial_json(json!({"method":"initialize"})))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0","id":body["id"],"result":{
                "protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"coverage-probe","version":"1.0"}
            }}))
        }).expect(2).mount(&server).await;
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"method":"notifications/initialized"}),
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&server)
        .await;
    Mock::given(method("POST")).and(body_partial_json(json!({"method":"tools/list"})))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().unwrap();
            let result = if fail_list {
                json!({"jsonrpc":"2.0","id":body["id"],"error":{"code":-32603,"message":"catalog unavailable"}})
            } else {
                json!({"jsonrpc":"2.0","id":body["id"],"result":{"tools":[
                    {"name":"search","description":"Search documents","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}}},"outputSchema":{"type":"object","properties":{"found":{"type":"boolean"}}}},
                    {"name":"ping","inputSchema":{"type":"object"}}
                ]}})
            };
            ResponseTemplate::new(200).set_body_json(result)
        }).expect(2).mount(&server).await;
    server
}

#[tokio::test]
async fn coverage_authenticated_mcp_probes_forward_identity_and_preserve_tool_schemas() {
    let server = server(false).await;
    let port = server.address().port();
    let token = SessionToken::new("fixture-session-token");
    let tools = agent::list_tools_authenticated("probe", port, &token, 2)
        .await
        .unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "search");
    assert_eq!(tools[0].parameters_count, 2);
    assert_eq!(tools[0].description.as_deref(), Some("Search documents"));
    assert_eq!(
        tools[0].input_schema.as_ref().unwrap()["properties"]["query"]["type"],
        "string"
    );
    assert_eq!(
        tools[0].output_schema.as_ref().unwrap()["properties"]["found"]["type"],
        "boolean"
    );
    assert_eq!(tools[1].parameters_count, 0);
    assert!(tools[1].output_schema.is_none());
    let other = plugin::list_tools_authenticated("probe", port, &token, 2)
        .await
        .unwrap();
    assert_eq!(other.len(), tools.len());
    for (a, b) in tools.iter().zip(&other) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.description, b.description);
        assert_eq!(a.parameters_count, b.parameters_count);
        assert_eq!(a.input_schema, b.input_schema);
        assert_eq!(a.output_schema, b.output_schema);
    }
    for request in server.received_requests().await.unwrap() {
        if request.method.as_str() == "POST" {
            assert_eq!(
                request.headers["authorization"],
                "Bearer fixture-session-token"
            );
        }
    }
}

#[tokio::test]
async fn coverage_public_mcp_probes_do_not_invent_a_bearer_credential() {
    let server = server(false).await;
    let port = server.address().port();
    let a = agent::list_tools_unauthenticated("public", port, 2)
        .await
        .unwrap();
    let b = plugin::list_tools_unauthenticated("public", port, 2)
        .await
        .unwrap();
    assert_eq!(a[0].name, "search");
    assert_eq!(b[0].parameters_count, 2);
    for request in server.received_requests().await.unwrap() {
        assert!(!request.headers.contains_key("authorization"));
    }
}

#[tokio::test]
async fn coverage_mcp_probe_tool_list_errors_are_not_reported_as_empty_catalogs() {
    let server = server(true).await;
    let token = SessionToken::new("fixture-session-token");
    let a = agent::list_tools_authenticated("probe", server.address().port(), &token, 2).await;
    let b = plugin::list_tools_authenticated("probe", server.address().port(), &token, 2).await;
    assert!(
        a.err()
            .unwrap()
            .to_string()
            .contains("Failed to list tools")
    );
    assert!(
        b.err()
            .unwrap()
            .to_string()
            .contains("Failed to list tools")
    );
}

#[tokio::test]
async fn coverage_mcp_probe_initialization_rejection_is_a_connection_failure() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let token = SessionToken::new("rejected");
    let a = agent::list_tools_authenticated("probe", server.address().port(), &token, 2).await;
    let b = plugin::list_tools_authenticated("probe", server.address().port(), &token, 2).await;
    assert!(a.err().unwrap().to_string().contains("Failed to connect"));
    assert!(b.err().unwrap().to_string().contains("Failed to connect"));
}
