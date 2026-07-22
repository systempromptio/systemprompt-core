//! `GET /executions/{id}` on the MCP proxy router against seeded execution
//! rows: the found path, malformed stored input, and unparseable output.

use axum::body::to_bytes;
use http::StatusCode;
use systemprompt_api::routes::proxy::mcp;
use systemprompt_database::DbPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, setup_ctx};

async fn seed_execution(
    pool: &DbPool,
    input: &str,
    output: Option<&str>,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let raw = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, input, output, status, user_id)
         VALUES ($1, 'lookup', 'files', CURRENT_TIMESTAMP, $2, $3, 'success', 'exec-user')",
    )
    .bind(&id)
    .bind(input)
    .bind(output)
    .execute(raw.as_ref())
    .await?;
    Ok(id)
}

#[tokio::test]
async fn get_execution_returns_row_with_derived_endpoint() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"query":"rust"}"#, Some(r#"{"hits":3}"#)).await?;
    let resp = mcp::router(&ctx)
        .oneshot(empty_get(&format!("/executions/{id}")))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), 1024 * 1024).await?)?;
    assert_eq!(body["id"].as_str(), Some(id.as_str()));
    assert_eq!(body["tool_name"].as_str(), Some("lookup"));
    assert_eq!(body["server_name"].as_str(), Some("files"));
    assert_eq!(body["input"]["query"].as_str(), Some("rust"));
    assert_eq!(body["output"]["hits"].as_i64(), Some(3));
    assert_eq!(body["status"].as_str(), Some("success"));
    assert!(
        body["server_endpoint"]
            .as_str()
            .is_some_and(|e| e.contains("files")),
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn get_execution_with_malformed_input_is_internal_error() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, "{not json", None).await?;
    let resp = mcp::router(&ctx)
        .oneshot(empty_get(&format!("/executions/{id}")))
        .await?;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn get_execution_with_unparseable_output_omits_output() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"a":1}"#, Some("{broken")).await?;
    let resp = mcp::router(&ctx)
        .oneshot(empty_get(&format!("/executions/{id}")))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), 1024 * 1024).await?)?;
    assert!(
        body.get("output").is_none(),
        "unparseable output must be skipped: {body}"
    );
    Ok(())
}
