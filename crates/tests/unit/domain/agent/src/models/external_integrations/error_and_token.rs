//! Unit tests for IntegrationError, TokenInfo, AuthorizationRequest/Result, and CallbackParams

use chrono::Utc;
use systemprompt_agent::models::external_integrations::{
    AuthorizationRequest, AuthorizationResult, CallbackParams, IntegrationError, TokenInfo,
};
use systemprompt_identifiers::AgentId;

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
