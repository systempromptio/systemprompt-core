// The affirmative arms of the A2A agent-card probe. The existing
// monitor_functions suite only drives its failure path, so the branch that
// accepts a well-formed agent card is never taken there.

use systemprompt_agent::services::agent_orchestration::monitor::check_a2a_agent_health;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn card_server(body: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    server
}

// Why: the probe must accept what the server actually serialises — an
// `AgentCard` carries `supportedInterfaces`, never a top-level `url`.
#[tokio::test]
async fn check_a2a_agent_health_accepts_a_serialised_agent_card() {
    let card = systemprompt_agent::models::a2a::AgentCard {
        name: "harness-agent".to_owned(),
        description: "harness".to_owned(),
        supported_interfaces: vec![systemprompt_models::a2a::AgentInterface {
            url: "http://127.0.0.1/a2a".to_owned(),
            protocol_binding: systemprompt_models::a2a::ProtocolBinding::JsonRpc,
            protocol_version: "0.3.0".to_owned(),
        }],
        version: "1.0.0".to_owned(),
        ..Default::default()
    };
    let server = card_server(serde_json::to_value(card).expect("card json")).await;

    assert!(
        check_a2a_agent_health(server.address().port(), 5)
            .await
            .expect("probe runs"),
        "a card advertising an interface is a healthy agent"
    );
}

#[tokio::test]
async fn check_a2a_agent_health_rejects_a_card_with_no_interfaces() {
    let server = card_server(serde_json::json!({
        "name": "harness-agent",
        "description": "harness",
        "supportedInterfaces": [],
        "version": "1.0.0",
        "capabilities": {},
        "defaultInputModes": [],
        "defaultOutputModes": [],
        "skills": []
    }))
    .await;

    assert!(
        !check_a2a_agent_health(server.address().port(), 5)
            .await
            .expect("probe runs"),
        "a 200 response is not enough — the card must advertise an interface"
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

// Why: a reachable server that answers the card path with an error status is a
// different outcome from an unreachable one, and only this arm distinguishes
// them. Treating a 500 as healthy would leave a wedged agent in the pool.
#[tokio::test]
async fn check_a2a_agent_health_rejects_a_reachable_server_that_answers_with_an_error_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let healthy = check_a2a_agent_health(server.address().port(), 5)
        .await
        .expect("a reachable server must still yield a verdict");

    assert!(
        !healthy,
        "a card endpoint answering 500 must not count as a healthy agent"
    );
}
