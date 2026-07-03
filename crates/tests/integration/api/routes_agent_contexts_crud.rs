//! Context CRUD (`create`/`update`/`get`) and artifact retrieval routes.
//!
//! Create is driven with a seeded session so the FK-backed insert succeeds
//! (201) plus the empty-name and whitespace-name 400 arms; update and get are
//! driven for the owned (200), foreign/unknown (404), and invalid-id (400)
//! branches. The artifacts router is exercised against a seeded, user-owned
//! artifact: list, single-get (owned 200 / foreign 403), and the App-UI
//! renderer (`table` renders, unknown type → 400).
//! `create_mcp_extensions_from_config` is unit-checked directly for its empty
//! and populated branches.

use axum::Extension;
use systemprompt_api::routes::agent::registry::create_mcp_extensions_from_config;
use systemprompt_api::routes::{artifacts_router, contexts_router};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use tower::ServiceExt;

use super::common::{empty_get, json_post, setup_ctx};

struct Owner {
    user_id: UserId,
    session_id: SessionId,
}

async fn seed_owner(pool: &DbPool) -> anyhow::Result<Owner> {
    let user_id = unique_user_id("crud");
    let session_id = SessionId::generate();
    let email = format!("{}@crud.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &session_id).await?;
    Ok(Owner {
        user_id,
        session_id,
    })
}

fn request_context_for(owner: &Owner) -> RequestContext {
    RequestContext::new(
        owner.session_id.clone(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("crud-agent"),
    )
    .with_actor(Actor::user(owner.user_id.clone()))
}

fn foreign_request_context(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("crud-agent"),
    )
    .with_actor(Actor::user(user.clone()))
}

async fn seed_context(pool: &DbPool, owner: &Owner) -> anyhow::Result<ContextId> {
    let context_id = ContextId::generate();
    let handle = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(context_id.as_str())
    .bind(owner.user_id.as_str())
    .bind(owner.session_id.as_str())
    .bind("crud-context")
    .execute(handle.as_ref())
    .await?;
    Ok(context_id)
}

fn contexts_app(ctx: &systemprompt_runtime::AppContext, rc: RequestContext) -> axum::Router {
    contexts_router()
        .with_state(ctx.clone())
        .layer(Extension(rc))
}

#[tokio::test]
async fn create_context_with_name_returns_created() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_post("/", serde_json::json!({ "name": "My Chat" })))
        .await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn create_context_without_name_defaults() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_post("/", serde_json::json!({})))
        .await?;
    assert_eq!(resp.status().as_u16(), 201, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn create_context_empty_name_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_post("/", serde_json::json!({ "name": "" })))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn create_context_whitespace_name_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_post("/", serde_json::json!({ "name": "   " })))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_context_owned_returns_200() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let context_id = seed_context(&pool, &owner).await?;
    let uri = format!("/{}", context_id.as_str());
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get(&uri))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_context_unknown_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let missing = ContextId::generate();
    let uri = format!("/{}", missing.as_str());
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get(&uri))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_context_invalid_id_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get("/undefined"))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

fn json_put(uri: &str, body: serde_json::Value) -> axum::http::Request<axum::body::Body> {
    axum::http::Request::builder()
        .method(http::Method::PUT)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .expect("request build")
}

#[tokio::test]
async fn update_context_owned_returns_200() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let context_id = seed_context(&pool, &owner).await?;
    let uri = format!("/{}", context_id.as_str());
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_put(&uri, serde_json::json!({ "name": "Renamed" })))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn update_context_foreign_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let context_id = seed_context(&pool, &owner).await?;
    let intruder = unique_user_id("crud-intruder");
    let uri = format!("/{}", context_id.as_str());
    let resp = contexts_app(&ctx, foreign_request_context(&intruder))
        .oneshot(json_put(&uri, serde_json::json!({ "name": "Hijack" })))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn update_context_invalid_id_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let resp = contexts_app(&ctx, request_context_for(&owner))
        .oneshot(json_put("/null", serde_json::json!({ "name": "x" })))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

async fn seed_artifact(
    pool: &DbPool,
    owner: &Owner,
    artifact_type: &str,
) -> anyhow::Result<ArtifactId> {
    let context_id = ContextId::generate();
    let task_id = TaskId::generate();
    let artifact_id = ArtifactId::generate();
    let handle = pool.pool_arc()?;

    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(context_id.as_str())
    .bind(owner.user_id.as_str())
    .bind(owner.session_id.as_str())
    .bind("art-context")
    .execute(handle.as_ref())
    .await?;

    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, \
         agent_name) VALUES ($1, $2, 'TASK_STATE_WORKING', now(), $3, 'art-agent')",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(owner.user_id.as_str())
    .execute(handle.as_ref())
    .await?;

    sqlx::query(
        "INSERT INTO task_artifacts (task_id, context_id, artifact_id, name, artifact_type) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(artifact_id.as_str())
    .bind("crud artifact")
    .bind(artifact_type)
    .execute(handle.as_ref())
    .await?;

    sqlx::query(
        "INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, \
         data_content) VALUES ($1, $2, 'data', 0, $3::jsonb)",
    )
    .bind(artifact_id.as_str())
    .bind(context_id.as_str())
    .bind(serde_json::json!({ "columns": ["name"], "rows": [["a"]] }).to_string())
    .execute(handle.as_ref())
    .await?;

    Ok(artifact_id)
}

fn artifacts_app(ctx: &systemprompt_runtime::AppContext, rc: RequestContext) -> axum::Router {
    artifacts_router()
        .with_state(ctx.clone())
        .layer(Extension(rc))
}

#[tokio::test]
async fn list_and_get_owned_artifact() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let artifact_id = seed_artifact(&pool, &owner, "table").await?;

    let list = artifacts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get("/"))
        .await?;
    assert_eq!(list.status().as_u16(), 200, "{}", list.status());

    let uri = format!("/{}", artifact_id.as_str());
    let got = artifacts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get(&uri))
        .await?;
    assert_eq!(got.status().as_u16(), 200, "{}", got.status());
    Ok(())
}

#[tokio::test]
async fn get_artifact_foreign_user_is_denied() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let artifact_id = seed_artifact(&pool, &owner, "table").await?;
    let intruder = unique_user_id("art-intruder");

    let uri = format!("/{}", artifact_id.as_str());
    let resp = artifacts_app(&ctx, foreign_request_context(&intruder))
        .oneshot(empty_get(&uri))
        .await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_artifact_ui_renders_table() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let artifact_id = seed_artifact(&pool, &owner, "table").await?;

    let uri = format!("/{}/ui", artifact_id.as_str());
    let resp = artifacts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get(&uri))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_artifact_ui_unknown_type_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_owner(&pool).await?;
    let artifact_id = seed_artifact(&pool, &owner, "no-renderer-type").await?;

    let uri = format!("/{}/ui", artifact_id.as_str());
    let resp = artifacts_app(&ctx, request_context_for(&owner))
        .oneshot(empty_get(&uri))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[test]
fn mcp_extensions_empty_when_no_servers() {
    let exts = create_mcp_extensions_from_config(&[], "https://example.test");
    assert!(exts.is_empty());
}

#[test]
fn mcp_extensions_built_for_named_servers() {
    let servers = vec!["alpha".to_owned(), "beta".to_owned()];
    let exts = create_mcp_extensions_from_config(&servers, "https://example.test");
    assert_eq!(exts.len(), 1);
    assert_eq!(exts[0].uri, "systemprompt:mcp-tools");
}
