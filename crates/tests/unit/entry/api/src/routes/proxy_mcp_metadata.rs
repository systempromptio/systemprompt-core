//! Serialization of the MCP reverse-proxy discovery metadata and
//! tool-execution response payloads (`routes::proxy::mcp`).
//!
//! These are pure `Serialize` DTOs; the handlers that build them need an
//! `AppContext`, so only the wire shape is exercised here.

use serde_json::Value;
use systemprompt_api::routes::proxy::mcp::{
    McpAuthorizationServerMetadata, McpProtectedResourceMetadata, ToolExecutionResponse,
};
use systemprompt_identifiers::McpExecutionId;

#[test]
fn protected_resource_metadata_serializes_all_fields() {
    let meta = McpProtectedResourceMetadata {
        resource: "https://gw.example/api/v1/mcp/foo/mcp".to_owned(),
        authorization_servers: vec!["https://gw.example".to_owned()],
        scopes_supported: vec!["user".to_owned(), "admin".to_owned()],
        bearer_methods_supported: vec!["header".to_owned()],
        resource_documentation: Some("https://gw.example".to_owned()),
    };
    let v: Value = serde_json::to_value(&meta).expect("serialize");
    assert_eq!(v["resource"], "https://gw.example/api/v1/mcp/foo/mcp");
    assert_eq!(v["authorization_servers"][0], "https://gw.example");
    assert_eq!(v["scopes_supported"].as_array().expect("array").len(), 2);
    assert_eq!(v["bearer_methods_supported"][0], "header");
    assert_eq!(v["resource_documentation"], "https://gw.example");
}

#[test]
fn protected_resource_metadata_serializes_null_documentation() {
    let meta = McpProtectedResourceMetadata {
        resource: "r".to_owned(),
        authorization_servers: vec![],
        scopes_supported: vec![],
        bearer_methods_supported: vec![],
        resource_documentation: None,
    };
    let v: Value = serde_json::to_value(&meta).expect("serialize");
    assert!(v["resource_documentation"].is_null());
    assert!(
        v["authorization_servers"]
            .as_array()
            .expect("array")
            .is_empty()
    );
}

#[test]
fn authorization_server_metadata_serializes_all_fields() {
    let meta = McpAuthorizationServerMetadata {
        issuer: "https://gw.example".to_owned(),
        authorization_endpoint: "https://gw.example/authorize".to_owned(),
        token_endpoint: "https://gw.example/token".to_owned(),
        registration_endpoint: Some("https://gw.example/register".to_owned()),
        scopes_supported: vec!["user".to_owned()],
        response_types_supported: vec!["code".to_owned()],
        grant_types_supported: vec!["authorization_code".to_owned(), "refresh_token".to_owned()],
        code_challenge_methods_supported: vec!["S256".to_owned()],
        token_endpoint_auth_methods_supported: vec!["none".to_owned()],
        authorization_response_iss_parameter_supported: true,
    };
    let v: Value = serde_json::to_value(&meta).expect("serialize");
    assert_eq!(v["issuer"], "https://gw.example");
    assert_eq!(v["authorization_endpoint"], "https://gw.example/authorize");
    assert_eq!(v["token_endpoint"], "https://gw.example/token");
    assert_eq!(v["registration_endpoint"], "https://gw.example/register");
    assert_eq!(v["response_types_supported"][0], "code");
    assert_eq!(
        v["grant_types_supported"].as_array().expect("array").len(),
        2
    );
    assert_eq!(v["code_challenge_methods_supported"][0], "S256");
    assert_eq!(v["authorization_response_iss_parameter_supported"], true);
}

#[test]
fn authorization_server_metadata_null_registration_endpoint() {
    let meta = McpAuthorizationServerMetadata {
        issuer: "i".to_owned(),
        authorization_endpoint: "a".to_owned(),
        token_endpoint: "t".to_owned(),
        registration_endpoint: None,
        scopes_supported: vec![],
        response_types_supported: vec![],
        grant_types_supported: vec![],
        code_challenge_methods_supported: vec![],
        token_endpoint_auth_methods_supported: vec![],
        authorization_response_iss_parameter_supported: false,
    };
    let v: Value = serde_json::to_value(&meta).expect("serialize");
    assert!(v["registration_endpoint"].is_null());
    assert_eq!(v["authorization_response_iss_parameter_supported"], false);
}

#[test]
fn tool_execution_response_serializes_with_output() {
    let resp = ToolExecutionResponse {
        id: McpExecutionId::generate(),
        tool_name: "search".to_owned(),
        server_name: "sharepoint".to_owned(),
        server_endpoint: "http://localhost:9000/mcp".to_owned(),
        input: serde_json::json!({"q": "rust"}),
        output: Some(serde_json::json!({"hits": 3})),
        status: "completed".to_owned(),
    };
    let v: Value = serde_json::to_value(&resp).expect("serialize");
    assert_eq!(v["tool_name"], "search");
    assert_eq!(v["server_name"], "sharepoint");
    assert_eq!(v["server_endpoint"], "http://localhost:9000/mcp");
    assert_eq!(v["input"]["q"], "rust");
    assert_eq!(v["output"]["hits"], 3);
    assert_eq!(v["status"], "completed");
    assert!(v["id"].is_string());
}

#[test]
fn tool_execution_response_omits_null_output() {
    let resp = ToolExecutionResponse {
        id: McpExecutionId::generate(),
        tool_name: "noop".to_owned(),
        server_name: "srv".to_owned(),
        server_endpoint: "http://localhost:1/mcp".to_owned(),
        input: serde_json::json!({}),
        output: None,
        status: "pending".to_owned(),
    };
    let v: Value = serde_json::to_value(&resp).expect("serialize");
    // `output` carries skip_serializing_if = "Option::is_none".
    assert!(v.get("output").is_none());
    assert_eq!(v["status"], "pending");
}
