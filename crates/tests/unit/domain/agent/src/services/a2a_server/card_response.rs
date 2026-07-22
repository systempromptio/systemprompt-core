// Exercises agent_card_response with injected registry snapshots: a registry
// containing the agent yields 200 with the assembled card, an unknown agent
// yields 404, and an injected registry-load failure yields 500.

use std::collections::HashMap;

use axum::http::StatusCode;
use systemprompt_agent::AgentError;
use systemprompt_agent::services::a2a_server::handlers::card::agent_card_response;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::ServicesConfig;

use super::a2a_helpers::agent_config;

fn registry_with(agent_name: &str) -> AgentRegistry {
    let mut agents = HashMap::new();
    agents.insert(agent_name.to_owned(), agent_config(agent_name));
    AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    })
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("read body");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

#[tokio::test]
async fn known_agent_returns_ok_with_card() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let response = agent_card_response(
        Ok(registry_with("card_seam_agent")),
        "card_seam_agent",
        "http://cards.invalid",
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert!(
        body.contains("test agent"),
        "card body must carry the agent description, got {body}"
    );
}

#[tokio::test]
async fn unknown_agent_returns_not_found() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let response = agent_card_response(
        Ok(registry_with("card_seam_agent")),
        "missing_agent",
        "http://cards.invalid",
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_string(response).await;
    assert!(body.contains("missing_agent"), "got {body}");
}

#[tokio::test]
async fn registry_failure_returns_internal_error() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let response = agent_card_response(
        Err(AgentError::Init("injected registry failure".to_owned())),
        "card_seam_agent",
        "http://cards.invalid",
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_string(response).await;
    assert!(
        body.contains("Failed to initialize agent registry"),
        "got {body}"
    );
}
