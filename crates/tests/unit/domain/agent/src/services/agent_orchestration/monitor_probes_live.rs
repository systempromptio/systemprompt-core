// The affirmative arms of the standalone monitor probes. The existing
// monitor_functions suite only drives their failure paths, so the branch that
// accepts a live listener and the branch that accepts a well-formed agent card
// are never taken.

use systemprompt_agent::services::agent_orchestration::monitor::{
    check_a2a_agent_health, check_agent_responsiveness,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// `check_agent_responsiveness` derives the port from the digits in the agent
// name as 8000 + (digits % 1000), so reaching a real listener means binding a
// port in that window and naming the agent after its offset.
fn listener_in_derived_range() -> Option<(std::net::TcpListener, String)> {
    let listener = systemprompt_test_fixtures::bind_in_range(8000..9000)?;
    let offset = listener.local_addr().ok()?.port() - 8000;
    Some((listener, format!("agent{offset}")))
}

async fn card_server(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn check_agent_responsiveness_is_true_when_the_derived_port_has_a_listener() {
    let Some((_listener, agent_name)) = listener_in_derived_range() else {
        return;
    };

    assert!(
        check_agent_responsiveness(&agent_name, 2)
            .await
            .expect("probe runs"),
        "a listener on the derived port makes {agent_name} responsive"
    );
}

#[tokio::test]
async fn check_a2a_agent_health_accepts_a_card_carrying_name_and_url() {
    let server = card_server(serde_json::json!({
        "name": "harness-agent",
        "url": "http://127.0.0.1/a2a",
        "version": "1.0.0"
    }))
    .await;

    assert!(
        check_a2a_agent_health(server.address().port(), 5)
            .await
            .expect("probe runs"),
        "a card with both name and url is a healthy agent"
    );
}

#[tokio::test]
async fn check_a2a_agent_health_rejects_a_card_missing_the_url_field() {
    let server = card_server(serde_json::json!({"name": "harness-agent"})).await;

    assert!(
        !check_a2a_agent_health(server.address().port(), 5)
            .await
            .expect("probe runs"),
        "a 200 response is not enough — the card must name both fields"
    );
}

#[tokio::test]
async fn check_a2a_agent_health_rejects_a_non_json_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    assert!(
        !check_a2a_agent_health(server.address().port(), 5)
            .await
            .expect("probe runs"),
        "an unparseable body is unhealthy rather than an error"
    );
}
