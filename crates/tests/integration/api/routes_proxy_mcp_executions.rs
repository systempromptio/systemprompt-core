//! `GET /executions/{id}` on the MCP executions router against seeded
//! execution rows: the owner and an admin read it, any other caller gets the
//! same 404 as a missing row, and malformed stored payloads are handled.

use axum::Extension;
use axum::body::to_bytes;
use http::StatusCode;
use systemprompt_api::routes::proxy::mcp;
use systemprompt_database::DbPool;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, request_context, setup_ctx};

const OWNER: &str = "exec-user";

async fn seed_execution(
    pool: &DbPool,
    input: &str,
    output: Option<&str>,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    let raw = pool.pool();
    sqlx::query(
        "INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, input, output, status, user_id)
         VALUES ($1, 'lookup', 'files', CURRENT_TIMESTAMP, $2, $3, 'success', $4)",
    )
    .bind(&id)
    .bind(input)
    .bind(output)
    .bind(OWNER)
    .execute(raw.as_ref())
    .await?;
    Ok(id)
}

fn caller(user: &str, user_type: UserType) -> RequestContext {
    request_context(user).with_user_type(user_type)
}

async fn get_as(
    ctx: &AppContext,
    id: &str,
    req_ctx: RequestContext,
) -> anyhow::Result<(StatusCode, String)> {
    let resp = mcp::executions_router(ctx)
        .layer(Extension(req_ctx))
        .oneshot(empty_get(&format!("/executions/{id}")))
        .await?;
    let status = resp.status();
    let body = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

#[tokio::test]
async fn owner_reads_execution_with_derived_endpoint() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"query":"rust"}"#, Some(r#"{"hits":3}"#)).await?;
    let (status, body) = get_as(&ctx, &id, caller(OWNER, UserType::User)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body)?;
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
async fn admin_reads_another_users_execution() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"a":1}"#, None).await?;
    let (status, body) = get_as(&ctx, &id, caller("some-admin", UserType::Admin)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    Ok(())
}

#[tokio::test]
async fn other_user_gets_the_same_404_as_a_missing_execution() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"secret":"value"}"#, Some(r#"{"x":1}"#)).await?;

    let (status, body) = get_as(&ctx, &id, caller("intruder", UserType::User)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(!body.contains("secret"), "{body}");

    let missing = Uuid::new_v4().to_string();
    let (missing_status, missing_body) =
        get_as(&ctx, &missing, caller("intruder", UserType::User)).await?;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_eq!(body, missing_body);
    Ok(())
}

#[tokio::test]
async fn anonymous_caller_with_owner_id_is_not_found() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"a":1}"#, None).await?;
    let (status, _) = get_as(&ctx, &id, caller(OWNER, UserType::Anon)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn malformed_input_is_internal_error_without_parser_detail() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, "{not json", None).await?;
    let (status, body) = get_as(&ctx, &id, caller(OWNER, UserType::User)).await?;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body.contains("line 1"), "{body}");
    Ok(())
}

#[tokio::test]
async fn unparseable_output_is_omitted() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let id = seed_execution(&pool, r#"{"a":1}"#, Some("{broken")).await?;
    let (status, body) = get_as(&ctx, &id, caller(OWNER, UserType::User)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let body: serde_json::Value = serde_json::from_str(&body)?;
    assert!(
        body.get("output").is_none(),
        "unparseable output must be skipped: {body}"
    );
    Ok(())
}
