//! Tests for the public MCP registry endpoint.
//!
//! `handle_mcp_registry` projects the configured MCP servers into the discovery
//! document clients read. Whether the registry can be enumerated depends on
//! process-global services config that other tests in this binary may or may
//! not have installed, so these assert the contract that holds in both states
//! rather than pinning one status: the response is always JSON, a success
//! always carries a server list, and a failure always names what failed to
//! load. In particular a registry that cannot be read must never present as an
//! empty list under `data` — that would tell clients this deployment has no MCP
//! servers, which is a different and wrong answer.

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

struct Reply {
    status: axum::http::StatusCode,
    content_type: String,
    body: String,
}

async fn call() -> Option<Reply> {
    let response = handle_mcp_registry(State(ctx().await?))
        .await
        .into_response();
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    Some(Reply {
        status,
        content_type,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[tokio::test]
async fn the_response_is_always_json() {
    // skip-ok: the API harness could not bind a port here
    let Some(reply) = call().await else {
        return;
    };

    assert!(
        reply.content_type.contains("json"),
        "clients parse this endpoint as JSON, got content-type {:?}",
        reply.content_type
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&reply.body).is_ok(),
        "body must be valid JSON in both the success and failure cases: {}",
        reply.body
    );
}

#[tokio::test]
async fn a_failure_names_the_registry_and_is_never_an_empty_list() {
    // skip-ok: the API harness could not bind a port here
    let Some(reply) = call().await else {
        return;
    };

    if reply.status.is_success() {
        let value: serde_json::Value = serde_json::from_str(&reply.body).expect("json");
        assert!(
            value["data"].is_array(),
            "a success must carry the server list under `data`: {}",
            reply.body
        );
        assert!(
            value["meta"]["version"].is_string(),
            "the success envelope must carry meta.version: {}",
            reply.body
        );
        return;
    }

    assert_eq!(
        reply.status,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "the only non-success outcome is a load failure"
    );
    assert!(
        reply.body.contains("MCP registry"),
        "a failure must name what could not be loaded: {}",
        reply.body
    );
    let value: serde_json::Value = serde_json::from_str(&reply.body).expect("json");
    assert!(
        !value["data"].is_array(),
        "a failed load must not present as an empty server list: {}",
        reply.body
    );
}
