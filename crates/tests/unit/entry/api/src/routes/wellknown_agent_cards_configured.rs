use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use systemprompt_api::routes::wellknown::agent_cards::wellknown_router;
use systemprompt_models::Config;
use systemprompt_test_fixtures::{
    fixture_app_context_with_config, fixture_db_pool, init_isolated_bootstrap,
};
use tower::ServiceExt;

const SERVICES: &str = r#"agents:
  configured_agent:
    name: configured_agent
    port: 9461
    endpoint: /api/v1/agents/configured_agent/
    enabled: true
    dev_only: false
    is_primary: true
    default: true
    tags: [governance]
    card:
      protocolVersion: 0.3.0
      name: Configured Agent
      displayName: Configured Agent
      description: Governed configured agent
      version: 2.1.0
      preferredTransport: HTTP+JSON
      capabilities:
        streaming: true
        pushNotifications: true
        stateTransitionHistory: true
      defaultInputModes: [text/plain]
      defaultOutputModes: [application/json]
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: Follow configured policy.
      mcpServers:
        source: explicit
      skills:
        source: explicit
      provider: anthropic
      model: governed-model
      toolModelOverrides: {}
    oauth:
      required: false
      scopes: []
      audience: a2a
"#;

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn configured_default_named_and_list_cards_share_the_public_agent_contract() {
    let boot = init_isolated_bootstrap("https://agents.example.test/base", SERVICES);
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let ctx = fixture_app_context_with_config(
        &pool,
        Config::get().expect("isolated profile config").clone(),
    )
    .unwrap();
    let app = wellknown_router(&ctx);

    let (default_status, default) = get(&app, "/.well-known/agent-card.json").await;
    let (named_status, named) = get(&app, "/.well-known/agent-cards/configured_agent.json").await;
    let (list_status, list) = get(&app, "/.well-known/agent-cards").await;

    assert_eq!(default_status, StatusCode::OK);
    assert_eq!(named_status, StatusCode::OK);
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(default, named);
    assert_eq!(list.as_array().unwrap(), std::slice::from_ref(&named));
    assert_eq!(named["name"], "configured_agent");
    assert_eq!(named["description"], "Governed configured agent");
    assert_eq!(named["version"], "2.1.0");
    assert_eq!(
        named["supportedInterfaces"][0]["protocolBinding"],
        "HTTP+JSON"
    );
    assert_eq!(named["supportedInterfaces"][0]["protocolVersion"], "0.3.0");
    assert_eq!(
        named["supportedInterfaces"][0]["url"],
        "https://agents.example.test/base/api/v1/agents/configured_agent"
    );
    assert_eq!(named["capabilities"]["streaming"], true);
    assert_eq!(named["capabilities"]["pushNotifications"], true);
    assert_eq!(
        named["defaultInputModes"],
        serde_json::json!(["text/plain"])
    );
    assert_eq!(
        named["defaultOutputModes"],
        serde_json::json!(["application/json"])
    );
    let extensions = named["capabilities"]["extensions"].as_array().unwrap();
    assert!(extensions.iter().any(|item| {
        item["uri"] == "systemprompt:agent-identity" && item["params"]["name"] == "configured_agent"
    }));
    assert!(extensions.iter().any(|item| {
        item["uri"] == "systemprompt:system-instructions"
            && item["params"]["systemPrompt"] == "Follow configured policy."
    }));
}
