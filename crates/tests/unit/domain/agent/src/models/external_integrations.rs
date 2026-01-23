//! Unit tests for external integrations models
//!
//! Tests cover:
//! - IntegrationError variants and Display implementations
//! - TokenInfo serialization/deserialization
//! - AuthorizationRequest serialization
//! - AuthorizationResult serialization
//! - CallbackParams serialization
//! - RegisteredMcpServer serialization
//! - ToolExecutionResult serialization
//! - WebhookEndpoint serialization
//! - WebhookRequest/WebhookResponse serialization

use chrono::Utc;
use std::collections::HashMap;
use systemprompt_agent::models::external_integrations::{
    AuthorizationRequest, AuthorizationResult, CallbackParams, IntegrationError,
    RegisteredMcpServer, TokenInfo, ToolExecutionResult, WebhookEndpoint, WebhookRequest,
    WebhookResponse,
};
use systemprompt_identifiers::{AgentId, McpServerId};

// ============================================================================
// IntegrationError Tests
// ============================================================================

#[test]
fn test_integration_error_oauth() {
    let error = IntegrationError::OAuth("Failed to authenticate".to_string());
    assert!(error.to_string().contains("OAuth error"));
    assert!(error.to_string().contains("Failed to authenticate"));
}

#[test]
fn test_integration_error_mcp() {
    let error = IntegrationError::Mcp("Connection refused".to_string());
    assert!(error.to_string().contains("MCP error"));
    assert!(error.to_string().contains("Connection refused"));
}

#[test]
fn test_integration_error_webhook() {
    let error = IntegrationError::Webhook("Delivery failed".to_string());
    assert!(error.to_string().contains("Webhook error"));
    assert!(error.to_string().contains("Delivery failed"));
}

#[test]
fn test_integration_error_oauth2() {
    let error = IntegrationError::OAuth2("Token exchange failed".to_string());
    assert!(error.to_string().contains("OAuth2 error"));
    assert!(error.to_string().contains("Token exchange failed"));
}

#[test]
fn test_integration_error_invalid_token() {
    let error = IntegrationError::InvalidToken;
    assert!(error.to_string().contains("Invalid token"));
}

#[test]
fn test_integration_error_token_expired() {
    let error = IntegrationError::TokenExpired;
    assert!(error.to_string().contains("Token expired"));
}

#[test]
fn test_integration_error_server_not_found() {
    let error = IntegrationError::ServerNotFound("mcp-server-1".to_string());
    assert!(error.to_string().contains("Server not found"));
    assert!(error.to_string().contains("mcp-server-1"));
}

#[test]
fn test_integration_error_tool_not_found() {
    let error = IntegrationError::ToolNotFound("my-tool".to_string());
    assert!(error.to_string().contains("Tool not found"));
    assert!(error.to_string().contains("my-tool"));
}

#[test]
fn test_integration_error_invalid_signature() {
    let error = IntegrationError::InvalidSignature;
    assert!(error.to_string().contains("Invalid signature"));
}

#[test]
fn test_integration_error_debug() {
    let error = IntegrationError::InvalidToken;
    let debug = format!("{:?}", error);
    assert!(debug.contains("InvalidToken"));
}

// ============================================================================
// TokenInfo Tests
// ============================================================================

#[test]
fn test_token_info_serialize() {
    let token = TokenInfo {
        access_token: "abc123".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_xyz".to_string()),
        scopes: vec!["read".to_string(), "write".to_string()],
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&token).unwrap();
    assert!(json.contains("abc123"));
    assert!(json.contains("Bearer"));
    assert!(json.contains("3600"));
    assert!(json.contains("refresh_xyz"));
}

#[test]
fn test_token_info_deserialize() {
    let json = r#"{
        "access_token": "token123",
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": "refresh123",
        "scopes": ["openid", "profile"],
        "created_at": "2024-01-01T00:00:00Z"
    }"#;

    let token: TokenInfo = serde_json::from_str(json).unwrap();
    assert_eq!(token.access_token, "token123");
    assert_eq!(token.token_type, "Bearer");
    assert_eq!(token.expires_in, Some(7200));
    assert_eq!(token.refresh_token, Some("refresh123".to_string()));
    assert_eq!(token.scopes.len(), 2);
}

#[test]
fn test_token_info_optional_fields() {
    let json = r#"{
        "access_token": "token",
        "token_type": "Bearer",
        "scopes": [],
        "created_at": "2024-01-01T00:00:00Z"
    }"#;

    let token: TokenInfo = serde_json::from_str(json).unwrap();
    assert!(token.expires_in.is_none());
    assert!(token.refresh_token.is_none());
}

#[test]
fn test_token_info_clone() {
    let token = TokenInfo {
        access_token: "token".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: None,
        refresh_token: None,
        scopes: vec![],
        created_at: Utc::now(),
    };

    let cloned = token.clone();
    assert_eq!(token.access_token, cloned.access_token);
}

// ============================================================================
// AuthorizationRequest Tests
// ============================================================================

#[test]
fn test_authorization_request_serialize() {
    let request = AuthorizationRequest {
        authorization_url: "https://auth.example.com/authorize".to_string(),
        state: "random_state_123".to_string(),
        expires_at: Utc::now(),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("https://auth.example.com/authorize"));
    assert!(json.contains("random_state_123"));
}

#[test]
fn test_authorization_request_deserialize() {
    let json = r#"{
        "authorization_url": "https://example.com/auth",
        "state": "state123",
        "expires_at": "2024-12-31T23:59:59Z"
    }"#;

    let request: AuthorizationRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.authorization_url, "https://example.com/auth");
    assert_eq!(request.state, "state123");
}

// ============================================================================
// AuthorizationResult Tests
// ============================================================================

#[test]
fn test_authorization_result_success() {
    let result = AuthorizationResult {
        agent_id: AgentId::from("agent-1"),
        provider: "github".to_string(),
        success: true,
        tokens: Some(TokenInfo {
            access_token: "token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scopes: vec![],
            created_at: Utc::now(),
        }),
        error: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("agent-1"));
    assert!(json.contains("github"));
    assert!(json.contains("true"));
}

#[test]
fn test_authorization_result_failure() {
    let result = AuthorizationResult {
        agent_id: AgentId::from("agent-2"),
        provider: "google".to_string(),
        success: false,
        tokens: None,
        error: Some("Access denied".to_string()),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("false"));
    assert!(json.contains("Access denied"));
}

// ============================================================================
// CallbackParams Tests
// ============================================================================

#[test]
fn test_callback_params_success() {
    let params = CallbackParams {
        code: Some("auth_code_123".to_string()),
        state: "state_456".to_string(),
        error: None,
        error_description: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("auth_code_123"));
    assert!(json.contains("state_456"));
}

#[test]
fn test_callback_params_error() {
    let params = CallbackParams {
        code: None,
        state: "state_789".to_string(),
        error: Some("access_denied".to_string()),
        error_description: Some("User denied access".to_string()),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("access_denied"));
    assert!(json.contains("User denied access"));
}

#[test]
fn test_callback_params_deserialize() {
    let json = r#"{
        "code": "code123",
        "state": "state123",
        "error": null,
        "error_description": null
    }"#;

    let params: CallbackParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.code, Some("code123".to_string()));
    assert_eq!(params.state, "state123");
    assert!(params.error.is_none());
}

// ============================================================================
// RegisteredMcpServer Tests
// ============================================================================

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

// ============================================================================
// ToolExecutionResult Tests
// ============================================================================

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

// ============================================================================
// WebhookEndpoint Tests
// ============================================================================

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

// ============================================================================
// WebhookRequest Tests
// ============================================================================

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

// ============================================================================
// WebhookResponse Tests
// ============================================================================

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

#[test]
fn test_webhook_response_deserialize() {
    let json = r#"{"status": 201, "body": {"id": "created"}}"#;
    let response: WebhookResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.status, 201);
    assert!(response.body.is_some());
}
