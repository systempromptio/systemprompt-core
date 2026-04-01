//! Unit tests for RegisteredMcpServer, ToolExecutionResult, WebhookEndpoint, WebhookRequest/Response

use std::collections::HashMap;
use systemprompt_agent::models::external_integrations::{
    RegisteredMcpServer, ToolExecutionResult, WebhookEndpoint, WebhookRequest, WebhookResponse,
};
use systemprompt_identifiers::McpServerId;
use chrono::Utc;

#[test]
fn test_registered_mcp_server_serialize() {
    let server = RegisteredMcpServer {
        id: McpServerId::new("server-1"),
        name: "Brave Search".to_string(),
        url: "http://localhost:3000".to_string(),
        status: "connected".to_string(),
        capabilities: vec!["tools".to_string(), "resources".to_string()],
        tools: vec![],
        discovered_at: Utc::now(),
        last_seen: Utc::now(),
    };

    let json = serde_json::to_string(&server).unwrap();
    assert!(json.contains("server-1"));
    assert!(json.contains("Brave Search"));
    assert!(json.contains("connected"));
}

#[test]
fn test_registered_mcp_server_deserialize() {
    let json = r#"{
        "id": "mcp-server",
        "name": "Test Server",
        "url": "http://localhost:8080",
        "status": "running",
        "capabilities": ["tools"],
        "tools": [],
        "discovered_at": "2024-01-01T00:00:00Z",
        "last_seen": "2024-01-01T12:00:00Z"
    }"#;

    let server: RegisteredMcpServer = serde_json::from_str(json).unwrap();
    assert_eq!(server.id.as_str(), "mcp-server");
    assert_eq!(server.name, "Test Server");
    assert_eq!(server.status, "running");
}

#[test]
fn test_tool_execution_result_serialize() {
    let result = ToolExecutionResult {
        tool_name: "brave_search".to_string(),
        server_id: McpServerId::new("brave-server"),
        result: serde_json::json!({"results": ["item1", "item2"]}),
        execution_time_ms: 150,
        metadata: Some(serde_json::json!({"query": "test"})),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("brave_search"));
    assert!(json.contains("brave-server"));
    assert!(json.contains("150"));
}

#[test]
fn test_tool_execution_result_without_metadata() {
    let result = ToolExecutionResult {
        tool_name: "file_read".to_string(),
        server_id: McpServerId::new("fs-server"),
        result: serde_json::json!("file contents"),
        execution_time_ms: 50,
        metadata: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("file_read"));
    assert!(json.contains("50"));
}

#[test]
fn test_webhook_endpoint_serialize() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer token".to_string());

    let endpoint = WebhookEndpoint {
        id: "webhook-1".to_string(),
        url: "https://webhook.example.com/hook".to_string(),
        events: vec!["task.completed".to_string(), "task.failed".to_string()],
        secret: Some("webhook_secret".to_string()),
        headers,
        active: true,
    };

    let json = serde_json::to_string(&endpoint).unwrap();
    assert!(json.contains("webhook-1"));
    assert!(json.contains("https://webhook.example.com/hook"));
    assert!(json.contains("task.completed"));
    assert!(json.contains("webhook_secret"));
}

#[test]
fn test_webhook_endpoint_without_secret() {
    let endpoint = WebhookEndpoint {
        id: "webhook-2".to_string(),
        url: "https://example.com/callback".to_string(),
        events: vec!["event.created".to_string()],
        secret: None,
        headers: HashMap::new(),
        active: false,
    };

    let json = serde_json::to_string(&endpoint).unwrap();
    assert!(json.contains("webhook-2"));
    assert!(json.contains("false"));
}

#[test]
fn test_webhook_request_serialize() {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let request = WebhookRequest {
        headers,
        body: serde_json::json!({"event": "task.completed", "task_id": "123"}),
        signature: Some("sha256=abc123".to_string()),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("Content-Type"));
    assert!(json.contains("task.completed"));
    assert!(json.contains("sha256=abc123"));
}

#[test]
fn test_webhook_request_without_signature() {
    let request = WebhookRequest {
        headers: HashMap::new(),
        body: serde_json::json!({"data": "test"}),
        signature: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("data"));
}

#[test]
fn test_webhook_response_success() {
    let response = WebhookResponse {
        status: 200,
        body: Some(serde_json::json!({"received": true})),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("200"));
    assert!(json.contains("received"));
}

#[test]
fn test_webhook_response_no_body() {
    let response = WebhookResponse {
        status: 204,
        body: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("204"));
}

#[test]
fn test_webhook_response_error() {
    let response = WebhookResponse {
        status: 500,
        body: Some(serde_json::json!({"error": "Internal server error"})),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("500"));
    assert!(json.contains("Internal server error"));
}
