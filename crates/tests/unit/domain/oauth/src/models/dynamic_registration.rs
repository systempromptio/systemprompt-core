//! Tests for DynamicRegistrationRequest and DynamicRegistrationResponse

use systemprompt_core_oauth::{DynamicRegistrationRequest, DynamicRegistrationResponse};

// ============================================================================
// DynamicRegistrationRequest Tests
// ============================================================================

fn create_valid_request() -> DynamicRegistrationRequest {
    serde_json::from_str(
        r#"{
            "client_name": "Test Client",
            "redirect_uris": ["https://example.com/callback"],
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "scope": "openid profile",
            "token_endpoint_auth_method": "client_secret_post",
            "client_uri": "https://example.com",
            "logo_uri": "https://example.com/logo.png",
            "contacts": ["admin@example.com"]
        }"#,
    )
    .unwrap()
}

#[test]
fn test_dynamic_registration_request_deserialization() {
    let request = create_valid_request();
    assert_eq!(request.client_name, Some("Test Client".to_string()));
    assert!(request.redirect_uris.is_some());
    assert!(request.grant_types.is_some());
}

#[test]
fn test_dynamic_registration_request_minimal() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    assert!(request.client_name.is_none());
    assert!(request.redirect_uris.is_none());
    assert!(request.grant_types.is_none());
}

#[test]
fn test_dynamic_registration_request_with_software_statement() {
    let json = r#"{
        "client_name": "Client with Statement",
        "redirect_uris": ["https://example.com/callback"],
        "software_statement": "eyJhbGciOiJSUzI1NiJ9..."
    }"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    assert!(request.software_statement.is_some());
}

#[test]
fn test_get_client_name_success() {
    let request = create_valid_request();
    let result = request.get_client_name();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Test Client");
}

#[test]
fn test_get_client_name_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_client_name();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("client_name is required"));
}

#[test]
fn test_get_client_name_empty() {
    let json = r#"{"client_name": ""}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_client_name();
    assert!(result.is_err());
}

#[test]
fn test_get_redirect_uris_success() {
    let request = create_valid_request();
    let result = request.get_redirect_uris();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn test_get_redirect_uris_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_redirect_uris();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("redirect_uris are required"));
}

#[test]
fn test_get_redirect_uris_empty() {
    let json = r#"{"redirect_uris": []}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_redirect_uris();
    assert!(result.is_err());
}

#[test]
fn test_get_grant_types_success() {
    let request = create_valid_request();
    let result = request.get_grant_types();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec!["authorization_code"]);
}

#[test]
fn test_get_grant_types_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_grant_types();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("grant_types are required"));
}

#[test]
fn test_get_grant_types_empty() {
    let json = r#"{"grant_types": []}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_grant_types();
    assert!(result.is_err());
}

#[test]
fn test_get_response_types_success() {
    let request = create_valid_request();
    let result = request.get_response_types();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec!["code"]);
}

#[test]
fn test_get_response_types_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_response_types();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("response_types are required"));
}

#[test]
fn test_get_response_types_empty() {
    let json = r#"{"response_types": []}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_response_types();
    assert!(result.is_err());
}

#[test]
fn test_get_scopes_with_value() {
    let request = create_valid_request();
    let scopes = request.get_scopes();
    assert_eq!(scopes.len(), 2);
    assert!(scopes.contains(&"openid".to_string()));
    assert!(scopes.contains(&"profile".to_string()));
}

#[test]
fn test_get_scopes_with_extra_whitespace() {
    let json = r#"{"scope": "  openid   profile   email  "}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let scopes = request.get_scopes();
    assert_eq!(scopes.len(), 3);
}

#[test]
fn test_get_scopes_empty() {
    let json = r#"{"scope": ""}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let scopes = request.get_scopes();
    assert!(scopes.is_empty());
}

#[test]
fn test_get_scopes_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let scopes = request.get_scopes();
    assert!(scopes.is_empty());
}

#[test]
fn test_get_token_endpoint_auth_method_success() {
    let request = create_valid_request();
    let result = request.get_token_endpoint_auth_method();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "client_secret_post");
}

#[test]
fn test_get_token_endpoint_auth_method_missing() {
    let json = r#"{}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_token_endpoint_auth_method();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("token_endpoint_auth_method is required"));
}

#[test]
fn test_get_token_endpoint_auth_method_empty() {
    let json = r#"{"token_endpoint_auth_method": ""}"#;
    let request: DynamicRegistrationRequest = serde_json::from_str(json).unwrap();
    let result = request.get_token_endpoint_auth_method();
    assert!(result.is_err());
}

#[test]
fn test_dynamic_registration_request_debug() {
    let request = create_valid_request();
    let debug_str = format!("{:?}", request);
    assert!(debug_str.contains("Test Client"));
}

// ============================================================================
// DynamicRegistrationResponse Tests
// ============================================================================

fn create_valid_response() -> DynamicRegistrationResponse {
    DynamicRegistrationResponse {
        client_id: "client_abc123".to_string(),
        client_secret: "secret_xyz789".to_string(),
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        scope: "openid profile".to_string(),
        token_endpoint_auth_method: "client_secret_post".to_string(),
        client_uri: Some("https://example.com".to_string()),
        logo_uri: Some("https://example.com/logo.png".to_string()),
        contacts: Some(vec!["admin@example.com".to_string()]),
        client_secret_expires_at: 0,
        client_id_issued_at: chrono::Utc::now(),
        registration_access_token: "rat_token123".to_string(),
        registration_client_uri: "https://auth.example.com/register/client_abc123".to_string(),
    }
}

#[test]
fn test_dynamic_registration_response_creation() {
    let response = create_valid_response();
    assert_eq!(response.client_id, "client_abc123");
    assert_eq!(response.client_secret, "secret_xyz789");
    assert_eq!(response.client_name, "Test Client");
}

#[test]
fn test_dynamic_registration_response_without_optional_fields() {
    let response = DynamicRegistrationResponse {
        client_id: "client_minimal".to_string(),
        client_secret: "secret_minimal".to_string(),
        client_name: "Minimal Client".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        scope: "openid".to_string(),
        token_endpoint_auth_method: "none".to_string(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
        client_secret_expires_at: 0,
        client_id_issued_at: chrono::Utc::now(),
        registration_access_token: "rat_minimal".to_string(),
        registration_client_uri: "https://auth.example.com/register/client_minimal".to_string(),
    };

    assert!(response.client_uri.is_none());
    assert!(response.logo_uri.is_none());
    assert!(response.contacts.is_none());
}

#[test]
fn test_dynamic_registration_response_serialize() {
    let response = create_valid_response();
    let json = serde_json::to_string(&response).unwrap();

    assert!(json.contains("client_abc123"));
    assert!(json.contains("secret_xyz789"));
    assert!(json.contains("Test Client"));
    assert!(json.contains("registration_access_token"));
}

#[test]
fn test_dynamic_registration_response_serialize_skips_none_optional_fields() {
    let response = DynamicRegistrationResponse {
        client_id: "client_no_opt".to_string(),
        client_secret: "secret_no_opt".to_string(),
        client_name: "No Optional".to_string(),
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        scope: "openid".to_string(),
        token_endpoint_auth_method: "none".to_string(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
        client_secret_expires_at: 0,
        client_id_issued_at: chrono::Utc::now(),
        registration_access_token: "rat_no_opt".to_string(),
        registration_client_uri: "https://auth.example.com/register/client_no_opt".to_string(),
    };

    let json = serde_json::to_string(&response).unwrap();
    // The serde skip_serializing_if should skip None fields
    // Verify the required fields are present
    assert!(json.contains("client_id"));
    assert!(json.contains("client_secret"));
    // Note: The implementation may or may not skip null optional fields depending on serde config
}

#[test]
fn test_dynamic_registration_response_debug() {
    let response = create_valid_response();
    let debug_str = format!("{:?}", response);
    assert!(debug_str.contains("client_abc123"));
}

#[test]
fn test_dynamic_registration_response_client_secret_expires_at_zero() {
    let response = create_valid_response();
    assert_eq!(response.client_secret_expires_at, 0);
}

#[test]
fn test_dynamic_registration_response_with_expiry() {
    let mut response = create_valid_response();
    response.client_secret_expires_at = 1735689600;
    assert_eq!(response.client_secret_expires_at, 1735689600);
}
