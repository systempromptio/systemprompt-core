//! Tests for the public MCP registry endpoint.
//!
//! `handle_mcp_registry` projects the configured MCP servers into the discovery
//! document clients read. With no services config loaded the registry cannot
//! enumerate servers, and the handler must surface that as a 500 rather than an
//! empty document — an empty list would tell clients "this deployment has no
//! MCP servers", which is a different and wrong answer.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::extract::State;
use axum::response::IntoResponse;
use systemprompt_api::routes::mcp::registry::handle_mcp_registry;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};

async fn ctx() -> Option<systemprompt_runtime::AppContext> {
    let url = fixture_database_url().ok()?;
    let pool = fixture_db_pool(&url).await.ok()?;
    Some((*fixture_app_context(&pool, &url).ok()?).clone())
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
async fn an_unloadable_registry_is_reported_as_a_server_error() {
    let Some(ctx) = ctx().await else {
        return;
    };

    let response = handle_mcp_registry(State(ctx)).await.into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "a registry that cannot enumerate servers must not masquerade as empty"
    );

    let body = body_text(response).await;
    assert!(
        body.contains("MCP registry"),
        "the error should name what failed to load: {body}"
    );
}

#[tokio::test]
async fn the_failure_response_is_json() {
    let Some(ctx) = ctx().await else {
        return;
    };

    let response = handle_mcp_registry(State(ctx)).await.into_response();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = body_text(response).await;

    assert!(
        content_type.contains("json"),
        "clients parse this endpoint as JSON, got content-type {content_type:?}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&body).is_ok(),
        "the error body must be valid JSON: {body}"
    );
}
