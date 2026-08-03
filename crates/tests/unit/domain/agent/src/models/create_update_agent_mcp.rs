// The `mcp_servers` arm of create/update agent validation, which the existing
// suite skips because it never supplies the field. Every named server is
// checked against the registry, so against the empty bootstrap registry the
// request is rejected and told what was actually available.

use serde_json::json;
use systemprompt_agent::models::web::{CreateAgentRequest, UpdateAgentRequest};

fn create_request(mcp_servers: serde_json::Value) -> CreateAgentRequest {
    serde_json::from_value(json!({
        "card": {
            "name": "mcp-agent",
            "description": "Test",
            "version": "1.0.0",
            "url": "http://localhost:8080"
        },
        "mcp_servers": mcp_servers
    }))
    .expect("should deserialize")
}

fn update_request(mcp_servers: serde_json::Value) -> UpdateAgentRequest {
    serde_json::from_value(json!({
        "card": {
            "name": "mcp-agent",
            "description": "Test",
            "version": "1.0.0",
            "url": "http://localhost:8080"
        },
        "mcp_servers": mcp_servers
    }))
    .expect("should deserialize")
}

#[tokio::test]
async fn create_rejects_an_mcp_server_that_is_not_in_the_registry() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;

    let err = create_request(json!(["no_such_mcp_server"]))
        .validate()
        .await
        .expect_err("an unknown MCP server is not assignable");

    assert!(
        err.contains("Invalid MCP server(s): no_such_mcp_server"),
        "the rejection names the offending server: {err}"
    );
    assert!(
        err.contains("Available servers:"),
        "the operator is told what they could have used instead: {err}"
    );
}

#[tokio::test]
async fn create_reports_every_unknown_server_not_just_the_first() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;

    let err = create_request(json!(["missing_one", "missing_two"]))
        .validate()
        .await
        .expect_err("unknown MCP servers are rejected");

    assert!(err.contains("missing_one"), "got: {err}");
    assert!(err.contains("missing_two"), "got: {err}");
}

#[tokio::test]
async fn create_with_an_empty_mcp_server_list_skips_the_registry_check() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;

    create_request(json!([]))
        .validate()
        .await
        .expect("an empty assignment list is valid");
}

#[tokio::test]
async fn update_rejects_an_mcp_server_that_is_not_in_the_registry() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;

    let err = update_request(json!(["no_such_mcp_server"]))
        .validate()
        .await
        .expect_err("an unknown MCP server is not assignable");

    assert!(
        err.contains("Invalid MCP server(s): no_such_mcp_server"),
        "the rejection names the offending server: {err}"
    );
}

#[tokio::test]
async fn update_with_an_empty_mcp_server_list_skips_the_registry_check() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _skills_fixture_read = crate::SKILLS_FIXTURE_LOCK.read().await;

    update_request(json!([]))
        .validate()
        .await
        .expect("an empty assignment list is valid");
}
