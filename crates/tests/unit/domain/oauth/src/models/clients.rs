//! Tests for OAuthClient and related types

use chrono::Utc;
use systemprompt_oauth::{
    ClientRelations, CreateOAuthClientRequest, OAuthClient, OAuthClientResponse, OAuthClientRow,
    UpdateOAuthClientRequest,
};

// ============================================================================
// OAuthClientRow Tests
// ============================================================================

fn create_test_client_row() -> OAuthClientRow {
    OAuthClientRow {
        client_id: "client_test123".to_string(),
        client_secret_hash: Some("hashed_secret".to_string()),
        client_name: "Test Client".to_string(),
        name: Some("Display Name".to_string()),
        token_endpoint_auth_method: Some("client_secret_post".to_string()),
        client_uri: Some("https://example.com".to_string()),
        logo_uri: Some("https://example.com/logo.png".to_string()),
        is_active: Some(true),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
        last_used_at: Some(Utc::now()),
    }
}

fn create_test_relations() -> ClientRelations {
    ClientRelations {
        redirect_uris: vec!["https://example.com/callback".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        response_types: vec!["code".to_string()],
        scopes: vec!["openid".to_string(), "profile".to_string()],
        contacts: Some(vec!["admin@example.com".to_string()]),
    }
}

#[test]
fn test_oauth_client_row_creation() {
    let row = create_test_client_row();
    assert_eq!(row.client_id, "client_test123");
    assert_eq!(row.client_name, "Test Client");
    assert!(row.is_active.unwrap());
}

#[test]
fn test_oauth_client_row_with_none_values() {
    let row = OAuthClientRow {
        client_id: "client_minimal".to_string(),
        client_secret_hash: None,
        client_name: "Minimal Client".to_string(),
        name: None,
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        is_active: None,
        created_at: None,
        updated_at: None,
        last_used_at: None,
    };

    assert_eq!(row.client_id, "client_minimal");
    assert!(row.client_secret_hash.is_none());
    assert!(row.is_active.is_none());
}

#[test]
fn test_oauth_client_row_clone() {
    let row = create_test_client_row();
    let cloned = row.clone();
    assert_eq!(row.client_id, cloned.client_id);
    assert_eq!(row.client_name, cloned.client_name);
}

#[test]
fn test_oauth_client_row_debug() {
    let row = create_test_client_row();
    let debug_str = format!("{:?}", row);
    assert!(debug_str.contains("client_test123"));
    assert!(debug_str.contains("Test Client"));
}

#[test]
fn test_oauth_client_row_serialize() {
    let row = create_test_client_row();
    let json = serde_json::to_string(&row).unwrap();
    assert!(json.contains("client_test123"));
    assert!(json.contains("Test Client"));
}

#[test]
fn test_oauth_client_row_deserialize() {
    let json = r#"{
        "client_id": "client_deser",
        "client_secret_hash": null,
        "client_name": "Deserialized Client",
        "name": null,
        "token_endpoint_auth_method": null,
        "client_uri": null,
        "logo_uri": null,
        "is_active": true,
        "created_at": null,
        "updated_at": null,
        "last_used_at": null
    }"#;

    let row: OAuthClientRow = serde_json::from_str(json).unwrap();
    assert_eq!(row.client_id, "client_deser");
    assert_eq!(row.client_name, "Deserialized Client");
    assert!(row.is_active.unwrap());
}

// ============================================================================
// ClientRelations Tests
// ============================================================================

#[test]
fn test_client_relations_creation() {
    let relations = create_test_relations();
    assert_eq!(relations.redirect_uris.len(), 1);
    assert_eq!(relations.grant_types.len(), 1);
    assert_eq!(relations.scopes.len(), 2);
}

#[test]
fn test_client_relations_with_empty_collections() {
    let relations = ClientRelations {
        redirect_uris: vec![],
        grant_types: vec![],
        response_types: vec![],
        scopes: vec![],
        contacts: None,
    };

    assert!(relations.redirect_uris.is_empty());
    assert!(relations.grant_types.is_empty());
    assert!(relations.contacts.is_none());
}

#[test]
fn test_client_relations_with_multiple_values() {
    let relations = ClientRelations {
        redirect_uris: vec![
            "https://example.com/callback".to_string(),
            "https://example.com/callback2".to_string(),
        ],
        grant_types: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        response_types: vec!["code".to_string()],
        scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
        contacts: Some(vec!["admin@example.com".to_string(), "dev@example.com".to_string()]),
    };

    assert_eq!(relations.redirect_uris.len(), 2);
    assert_eq!(relations.grant_types.len(), 2);
    assert_eq!(relations.scopes.len(), 3);
    assert_eq!(relations.contacts.as_ref().unwrap().len(), 2);
}

#[test]
fn test_client_relations_debug() {
    let relations = create_test_relations();
    let debug_str = format!("{:?}", relations);
    assert!(debug_str.contains("redirect_uris"));
    assert!(debug_str.contains("grant_types"));
}

// ============================================================================
// OAuthClient Tests
// ============================================================================

#[test]
fn test_oauth_client_from_row_with_relations() {
    let row = create_test_client_row();
    let relations = create_test_relations();

    let client = OAuthClient::from_row_with_relations(row, relations);

    assert_eq!(client.client_id, "client_test123");
    assert_eq!(client.client_name, "Test Client");
    assert_eq!(client.name, Some("Display Name".to_string()));
    assert_eq!(client.redirect_uris.len(), 1);
    assert_eq!(client.grant_types.len(), 1);
    assert_eq!(client.scopes.len(), 2);
    assert!(client.is_active);
}

#[test]
fn test_oauth_client_from_row_with_default_values() {
    let row = OAuthClientRow {
        client_id: "client_defaults".to_string(),
        client_secret_hash: None,
        client_name: "Default Client".to_string(),
        name: None,
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        is_active: None,
        created_at: None,
        updated_at: None,
        last_used_at: None,
    };

    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    assert_eq!(client.token_endpoint_auth_method, "client_secret_post");
    assert!(client.is_active);
}

#[test]
fn test_oauth_client_validate_success() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_ok());
}

#[test]
fn test_oauth_client_validate_empty_client_id() {
    let mut row = create_test_client_row();
    row.client_id = String::new();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("client_id"));
}

#[test]
fn test_oauth_client_validate_empty_client_name() {
    let mut row = create_test_client_row();
    row.client_name = String::new();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("client_name"));
}

#[test]
fn test_oauth_client_validate_empty_redirect_uris() {
    let row = create_test_client_row();
    let mut relations = create_test_relations();
    relations.redirect_uris = vec![];
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("redirect_uris"));
}

#[test]
fn test_oauth_client_validate_empty_grant_types() {
    let row = create_test_client_row();
    let mut relations = create_test_relations();
    relations.grant_types = vec![];
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("grant_types"));
}

#[test]
fn test_oauth_client_validate_empty_response_types() {
    let row = create_test_client_row();
    let mut relations = create_test_relations();
    relations.response_types = vec![];
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("response_types"));
}

#[test]
fn test_oauth_client_validate_empty_scopes() {
    let row = create_test_client_row();
    let mut relations = create_test_relations();
    relations.scopes = vec![];
    let client = OAuthClient::from_row_with_relations(row, relations);

    let result = client.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("scopes"));
}

#[test]
fn test_oauth_client_clone() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let cloned = client.clone();
    assert_eq!(client.client_id, cloned.client_id);
    assert_eq!(client.scopes, cloned.scopes);
}

#[test]
fn test_oauth_client_serialize() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let json = serde_json::to_string(&client).unwrap();
    assert!(json.contains("client_test123"));
    assert!(json.contains("openid"));
}

#[test]
fn test_oauth_client_debug() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let debug_str = format!("{:?}", client);
    assert!(debug_str.contains("client_test123"));
}

// ============================================================================
// CreateOAuthClientRequest Tests
// ============================================================================

#[test]
fn test_create_oauth_client_request_deserialization() {
    let json = r#"{
        "client_id": "client_new",
        "name": "New Client",
        "redirect_uris": ["https://example.com/callback"],
        "scopes": ["openid", "profile"]
    }"#;

    let request: CreateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.client_id, "client_new");
    assert_eq!(request.name, "New Client");
    assert_eq!(request.redirect_uris.len(), 1);
    assert_eq!(request.scopes.len(), 2);
}

#[test]
fn test_create_oauth_client_request_debug() {
    let json = r#"{
        "client_id": "client_debug",
        "name": "Debug Client",
        "redirect_uris": ["https://example.com/callback"],
        "scopes": ["openid"]
    }"#;

    let request: CreateOAuthClientRequest = serde_json::from_str(json).unwrap();
    let debug_str = format!("{:?}", request);
    assert!(debug_str.contains("client_debug"));
}

// ============================================================================
// UpdateOAuthClientRequest Tests
// ============================================================================

#[test]
fn test_update_oauth_client_request_full() {
    let json = r#"{
        "name": "Updated Name",
        "redirect_uris": ["https://example.com/new-callback"],
        "scopes": ["openid", "email"]
    }"#;

    let request: UpdateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.name, Some("Updated Name".to_string()));
    assert!(request.redirect_uris.is_some());
    assert!(request.scopes.is_some());
}

#[test]
fn test_update_oauth_client_request_partial() {
    let json = r#"{
        "name": "Only Name Update"
    }"#;

    let request: UpdateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.name, Some("Only Name Update".to_string()));
    assert!(request.redirect_uris.is_none());
    assert!(request.scopes.is_none());
}

#[test]
fn test_update_oauth_client_request_empty() {
    let json = r#"{}"#;

    let request: UpdateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert!(request.name.is_none());
    assert!(request.redirect_uris.is_none());
    assert!(request.scopes.is_none());
}

#[test]
fn test_update_oauth_client_request_debug() {
    let json = r#"{"name": "Debug Update"}"#;

    let request: UpdateOAuthClientRequest = serde_json::from_str(json).unwrap();
    let debug_str = format!("{:?}", request);
    assert!(debug_str.contains("Debug Update"));
}

// ============================================================================
// OAuthClientResponse Tests
// ============================================================================

#[test]
fn test_oauth_client_response_from_client() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let response: OAuthClientResponse = client.into();

    assert_eq!(response.client_id, "client_test123");
    assert_eq!(response.name, "Display Name");
    assert_eq!(response.redirect_uris.len(), 1);
    assert_eq!(response.scopes.len(), 2);
}

#[test]
fn test_oauth_client_response_from_client_without_display_name() {
    let mut row = create_test_client_row();
    row.name = None;
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let response: OAuthClientResponse = client.into();

    assert_eq!(response.name, "Test Client");
}

#[test]
fn test_oauth_client_response_serialize() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);
    let response: OAuthClientResponse = client.into();

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("client_test123"));
    assert!(json.contains("Display Name"));
}

#[test]
fn test_oauth_client_response_debug() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);
    let response: OAuthClientResponse = client.into();

    let debug_str = format!("{:?}", response);
    assert!(debug_str.contains("client_test123"));
}
