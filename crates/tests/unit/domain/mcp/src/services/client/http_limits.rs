use futures::StreamExt;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::services::{McpConfig, ResilienceSettings};
use systemprompt_mcp::services::client::{
    HttpClientWithContext, McpClient, McpClientLimits, McpTransportError,
};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const SHORT: Duration = Duration::from_millis(300);
const TEST_BUDGET: Duration = Duration::from_secs(10);

fn short_limits() -> McpClientLimits {
    McpClientLimits::from(&ResilienceSettings {
        request_timeout_ms: 300,
        stream_idle_timeout_ms: 300,
        ..McpConfig::default().resilience
    })
}

fn client() -> HttpClientWithContext {
    let context = RequestContext::new(
        SessionId::new("s-limits"),
        TraceId::new("t-limits"),
        ContextId::generate(),
        AgentName::try_new("agent-limits").expect("valid AgentName"),
        Actor::user(UserId::new("user-limits")),
    );
    HttpClientWithContext::forwarding(context, HashMap::new(), short_limits())
        .expect("guarded client")
}

fn ping() -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
        .expect("valid jsonrpc request")
}

async fn raw_server(head: &'static str, body: Vec<u8>) -> Arc<str> {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0_u8; 8192];
        let _read = socket.read(&mut buf).await;
        let _head = socket.write_all(head.as_bytes()).await;
        let _body = socket.write_all(&body).await;
        let _flush = socket.flush().await;
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    Arc::from(format!("http://{addr}/mcp"))
}

async fn post(
    uri: Arc<str>,
    max: usize,
) -> Result<StreamableHttpPostResponse, StreamableHttpError<McpTransportError>> {
    tokio::time::timeout(
        TEST_BUDGET,
        client().post_message_with_max_sse_event_size(uri, ping(), None, None, HashMap::new(), max),
    )
    .await
    .expect("client call must finish within its own budget")
}

#[tokio::test]
async fn json_body_over_the_declared_limit_is_refused() {
    let server = MockServer::start().await;
    let body = format!(
        r#"{{"jsonrpc":"2.0","id":1,"result":{{"pad":"{}"}}}}"#,
        "x".repeat(4096)
    );
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(&server)
        .await;

    let result = post(Arc::from(format!("{}/mcp", server.uri())), 1024).await;
    assert!(
        matches!(
            result,
            Err(StreamableHttpError::Client(
                McpTransportError::ResponseTooLarge { limit: 1024 }
            ))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn chunked_json_body_over_the_limit_is_refused_without_content_length() {
    let mut body = Vec::new();
    for _ in 0..8 {
        body.extend_from_slice(format!("{:x}\r\n", 512).as_bytes());
        body.extend_from_slice(&[b' '; 512]);
        body.extend_from_slice(b"\r\n");
    }
    let uri = raw_server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ntransfer-encoding: chunked\r\n\r\n",
        body,
    )
    .await;

    let result = post(uri, 1024).await;
    assert!(
        matches!(
            result,
            Err(StreamableHttpError::Client(
                McpTransportError::ResponseTooLarge { limit: 1024 }
            ))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn json_body_within_the_limit_still_parses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
            "application/json",
        ))
        .mount(&server)
        .await;

    let result = post(Arc::from(format!("{}/mcp", server.uri())), 1024).await;
    assert!(
        matches!(result, Ok(StreamableHttpPostResponse::Json(..))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn stalled_response_headers_time_out() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(30))
                .set_body_raw(
                    r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
                    "application/json",
                ),
        )
        .mount(&server)
        .await;

    let result = post(Arc::from(format!("{}/mcp", server.uri())), 1024).await;
    assert!(
        matches!(
            result,
            Err(StreamableHttpError::Client(McpTransportError::TimedOut { after })) if after == SHORT
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn trickled_json_body_times_out() {
    let uri = raw_server(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 500\r\n\r\n",
        b"{\"jsonrpc\":".to_vec(),
    )
    .await;

    let result = post(uri, 1024).await;
    assert!(
        matches!(
            result,
            Err(StreamableHttpError::Client(
                McpTransportError::TimedOut { .. }
            ))
        ),
        "got {result:?}"
    );
}

#[tokio::test]
async fn silent_sse_stream_fails_after_the_idle_timeout() {
    let uri = raw_server(
        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n",
        Vec::new(),
    )
    .await;

    let response = post(uri, 1024).await.expect("sse response");
    let StreamableHttpPostResponse::Sse(mut stream, _) = response else {
        panic!("expected an SSE response");
    };
    let next = tokio::time::timeout(TEST_BUDGET, stream.next())
        .await
        .expect("idle stream must end within its idle budget");
    assert!(matches!(next, Some(Err(_))), "got {next:?}");
}

#[test]
fn default_limits_follow_the_mcp_resilience_profile_defaults() {
    let resilience = McpConfig::default().resilience;
    let limits = McpClientLimits::default();
    assert_eq!(
        limits.request_timeout(),
        Duration::from_millis(resilience.request_timeout_ms)
    );
    assert_eq!(limits, McpClientLimits::from(&resilience));
}

#[tokio::test]
async fn tool_call_through_mcp_client_honours_the_configured_request_timeout() {
    let _ = ensure_test_bootstrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(30))
                .set_body_raw(
                    r#"{"jsonrpc":"2.0","id":0,"result":{}}"#,
                    "application/json",
                ),
        )
        .mount(&server)
        .await;
    let config =
        crate::harness::external_mcp_config("limits_stall", &format!("{}/mcp", server.uri()));

    let started = std::time::Instant::now();
    let result = tokio::time::timeout(
        TEST_BUDGET,
        McpClient::new(
            Arc::default(),
            systemprompt_mcp::services::DeploymentService::new(
                crate::harness::services_state().view(),
            ),
            crate::harness::config(),
            systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
            short_limits(),
        )
        .call_tool(
            &config,
            "echo".to_owned(),
            None,
            &crate::harness::request_context("limits"),
        ),
    )
    .await
    .expect("tool call must finish within the configured deadline");

    assert!(result.is_err(), "a stalled server cannot produce a result");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}
