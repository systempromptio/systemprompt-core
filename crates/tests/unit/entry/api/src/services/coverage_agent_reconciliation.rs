use systemprompt_api::services::server::lifecycle::agents::reconcile_agents;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_db_pool, init_services_bootstrap, install_test_signing_key,
};

fn agent_yaml(name: &str, port: u16, display: &str, enabled: bool) -> String {
    format!(
        r#"agents:
  {name}:
    name: {name}
    port: {port}
    endpoint: /api/v1/agents/{name}/
    enabled: {enabled}
    dev_only: false
    is_primary: false
    default: false
    tags: []
    card:
      protocolVersion: 0.3.0
      name: {name}
      displayName: {display}
      description: Fixture agent for coverage
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
        pushNotifications: false
        stateTransitionHistory: true
      defaultInputModes:
      - text/plain
      defaultOutputModes:
      - text/plain
      supportsAuthenticatedExtendedCard: false
    metadata:
      systemPrompt: You are a fixture agent.
      mcpServers:
        source: instance
      skills:
        source: instance
      provider: anthropic
      model: claude-sonnet-4-5
      toolModelOverrides: {{}}
    oauth:
      required: false
      scopes: []
      audience: a2a
"#
    )
}

#[tokio::test]
async fn coverage_required_agent_failure_is_retried_and_blocks_api_startup() {
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("reconcile_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Required", true));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).unwrap();
    let err = reconcile_agents(&ctx, None).await.unwrap_err();
    let message = err.to_string();
    assert!(message.contains("failed to start after retry"), "{message}");
    assert!(message.contains(&name), "{message}");
    assert!(message.contains("port"), "{message}");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
}

#[tokio::test]
async fn coverage_disabled_agent_does_not_block_api_startup_on_an_occupied_port() {
    let listener = (9000..=9999)
        .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let name = format!("disabled_{}", uuid::Uuid::new_v4().simple());
    let boot = init_services_bootstrap(&agent_yaml(&name, port, "Disabled", false));
    install_test_signing_key();
    let pool = fixture_db_pool(&boot.database_url).await.unwrap();
    let ctx = fixture_app_context(&pool, &boot.database_url).unwrap();
    assert_eq!(reconcile_agents(&ctx, None).await.unwrap(), 0);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
}
