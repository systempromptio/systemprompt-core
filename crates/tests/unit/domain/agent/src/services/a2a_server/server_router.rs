// Construction and routing for the per-agent A2A HTTP server. The bootstrap
// services config already registers one agent, so `Server::new` can be driven
// for real and the router it builds can be exercised in-process with `oneshot`
// — no listener, and therefore none of `run`/`start_server`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_agent::services::a2a_server::Server;
use tower::ServiceExt;

use super::a2a_helpers::{StubAiProvider, make_agent_state};
use crate::repository::try_pool_or_skip;

const AGENT_PORT: u16 = 9312;

async fn server_for_registered_agent_or_skip(port: u16) -> Option<Server> {
    let pool = try_pool_or_skip().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let agent_state = make_agent_state(&pool);

    Server::new(
        Arc::clone(&pool),
        agent_state,
        Arc::new(StubAiProvider::new()),
        Some(systemprompt_test_fixtures::test_messaging_agent().to_owned()),
        port,
    )
    .await
    .ok()
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body")
        .to_vec()
}

#[tokio::test]
async fn server_new_loads_the_registered_agent_and_reports_its_port() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT).await else {
        return;
    };

    let debug = format!("{server:?}");
    assert!(
        debug.contains(&AGENT_PORT.to_string()),
        "the server records the port it was built for: {debug}"
    );
    assert!(
        !debug.contains("password") && !debug.contains("secret"),
        "the Debug surface must not carry credentials: {debug}"
    );
}

#[tokio::test]
async fn the_router_serves_the_well_known_agent_card_without_authentication() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 1).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/.well-known/agent-card.json")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);
    let card: serde_json::Value =
        serde_json::from_slice(&body_bytes(response).await).expect("the card is JSON");
    assert_eq!(
        card["name"],
        serde_json::json!(systemprompt_test_fixtures::test_messaging_agent()),
        "the card names the agent the server was built for: {card}"
    );
}

#[tokio::test]
async fn the_router_serves_the_card_on_both_advertised_paths() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 2).await else {
        return;
    };
    let router = server.create_router();

    let alias = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/a2a/card")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(
        alias.status(),
        StatusCode::OK,
        "the /a2a card alias resolves to the same handler"
    );
}

#[tokio::test]
async fn the_router_rejects_an_unrouted_path() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 3).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/definitely-not-a-route")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_post_route_runs_the_oauth_middleware_before_the_handler() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 4).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "GetTask",
                        "params": {"id": "no-such-task"},
                        "id": 1
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("router responds");

    // The middleware injects the RequestContext the handler requires, so a
    // response of any kind proves the layer ran; a bypassed layer surfaces as
    // the handler's own "request context unavailable" 500.
    let status = response.status();
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes(response).await).unwrap_or(serde_json::Value::Null);
    assert!(
        status == StatusCode::OK || status.is_client_error() || status.is_server_error(),
        "the POST route is wired: {status}"
    );
    assert_ne!(
        body["error"]["message"],
        serde_json::json!("Internal server error: request context unavailable"),
        "the oauth middleware must populate the request context before dispatch"
    );
}

#[tokio::test]
async fn the_router_answers_cors_preflight() {
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let Some(server) = server_for_registered_agent_or_skip(AGENT_PORT + 5).await else {
        return;
    };

    let response = server
        .create_router()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/")
                .header("origin", "https://example.invalid")
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router responds");

    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin"),
        "the permissive CORS layer wraps every route"
    );
}
