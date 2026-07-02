//! External-MCP audit tap — drives `audit::record` end-to-end via the
//! `test-api` seam with wiremock upstream responses. Verifies the body is
//! forwarded verbatim for both JSON and SSE upstreams and that an
//! `mcp_tool_executions` row is finalized with the matched outcome.

use axum::body::to_bytes;
use systemprompt_api::services::proxy::test_api::record_tool_call;
use systemprompt_database::DbPool;
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{request_context, setup_ctx};

fn tool_call_body(tool: &str) -> Vec<u8> {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {"name": tool, "arguments": {"q": "x"}}
    })
    .to_string()
    .into_bytes()
}

async fn upstream(template: ResponseTemplate) -> reqwest::Response {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(template)
        .mount(&server)
        .await;
    reqwest::Client::new()
        .post(server.uri())
        .send()
        .await
        .expect("upstream response")
}

async fn wait_for_execution_row(pool: &DbPool, tool: &str) -> Option<(String, Option<String>)> {
    let p = pool.pool_arc().expect("read pool");
    for _ in 0..100 {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT status, error_message FROM mcp_tool_executions WHERE tool_name = $1",
        )
        .bind(tool)
        .fetch_optional(p.as_ref())
        .await
        .expect("query executions");
        if row.is_some() {
            return row;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

#[tokio::test]
async fn json_response_is_forwarded_and_audited_as_success() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let tool = format!("tap-json-{}", Uuid::new_v4().simple());
    let upstream_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "result": {"content": [{"type": "text", "text": "ok"}]}
    })
    .to_string();
    let response = upstream(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_raw(upstream_body.clone(), "application/json"),
    )
    .await;

    let rc = request_context("tap-user");
    let out = record_tool_call(response, &pool, rc, "ext-server", &tool_call_body(&tool))
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(out.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(out.into_body(), 1024 * 1024).await?;
    assert_eq!(String::from_utf8_lossy(&bytes), upstream_body);

    let (status, error) = wait_for_execution_row(&pool, &tool)
        .await
        .expect("execution row written");
    assert!(error.is_none(), "unexpected error: {error:?}");
    assert!(!status.is_empty());
    Ok(())
}

#[tokio::test]
async fn json_error_frame_is_audited_with_error_message() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let tool = format!("tap-err-{}", Uuid::new_v4().simple());
    let upstream_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "error": {"code": -32000, "message": "tool exploded"}
    })
    .to_string();
    let response = upstream(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_raw(upstream_body, "application/json"),
    )
    .await;

    let rc = request_context("tap-user");
    record_tool_call(response, &pool, rc, "ext-server", &tool_call_body(&tool))
        .await
        .map_err(anyhow::Error::msg)?;

    let (_status, error) = wait_for_execution_row(&pool, &tool)
        .await
        .expect("execution row written");
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("tool exploded")),
        "got {error:?}"
    );
    Ok(())
}

#[tokio::test]
async fn sse_response_is_forwarded_and_matched_frame_audited() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let tool = format!("tap-sse-{}", Uuid::new_v4().simple());
    let frame = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "result": {"structuredContent": {"answer": 42}}
    })
    .to_string();
    let sse_body = format!("event: message\ndata: {frame}\n\n");
    let response = upstream(
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(sse_body.clone(), "text/event-stream"),
    )
    .await;

    let rc = request_context("tap-user");
    let out = record_tool_call(response, &pool, rc, "ext-server", &tool_call_body(&tool))
        .await
        .map_err(anyhow::Error::msg)?;
    assert_eq!(out.status(), axum::http::StatusCode::OK);
    let bytes = to_bytes(out.into_body(), 1024 * 1024).await?;
    assert!(
        String::from_utf8_lossy(&bytes).contains(&frame),
        "SSE body must be forwarded verbatim"
    );

    let (_status, error) = wait_for_execution_row(&pool, &tool)
        .await
        .expect("execution row written");
    assert!(error.is_none(), "unexpected error: {error:?}");
    Ok(())
}

#[tokio::test]
async fn sse_stream_without_matching_frame_finalizes_as_unparseable() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let tool = format!("tap-nomatch-{}", Uuid::new_v4().simple());
    let sse_body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":999,\"result\":{}}\n\n";
    let response = upstream(
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(sse_body, "text/event-stream"),
    )
    .await;

    let rc = request_context("tap-user");
    let out = record_tool_call(response, &pool, rc, "ext-server", &tool_call_body(&tool))
        .await
        .map_err(anyhow::Error::msg)?;
    to_bytes(out.into_body(), 1024 * 1024).await?;

    let (_status, error) = wait_for_execution_row(&pool, &tool)
        .await
        .expect("execution row written");
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("no parseable result")),
        "got {error:?}"
    );
    Ok(())
}

#[tokio::test]
async fn non_utf8_json_body_is_forwarded_but_not_parsed() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let tool = format!("tap-binary-{}", Uuid::new_v4().simple());
    let response = upstream(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_bytes(vec![0xff, 0xfe, 0x00]),
    )
    .await;

    let rc = request_context("tap-user");
    let out = record_tool_call(response, &pool, rc, "ext-server", &tool_call_body(&tool))
        .await
        .map_err(anyhow::Error::msg)?;
    let bytes = to_bytes(out.into_body(), 1024).await?;
    assert_eq!(bytes.as_ref(), &[0xff, 0xfe, 0x00]);

    let (_status, error) = wait_for_execution_row(&pool, &tool)
        .await
        .expect("execution row written");
    assert!(error.is_some());
    Ok(())
}

#[tokio::test]
async fn non_tool_call_request_body_is_rejected() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let response = upstream(ResponseTemplate::new(200)).await;
    let rc = request_context("tap-user");
    let body = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let result = record_tool_call(response, &pool, rc, "ext-server", body).await;
    assert!(result.is_err());
    Ok(())
}
