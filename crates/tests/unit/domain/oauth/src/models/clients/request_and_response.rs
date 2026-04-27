//! Tests for CreateOAuthClientRequest, UpdateOAuthClientRequest,
//! OAuthClientResponse

use chrono::Utc;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::{
    ClientRelations, CreateOAuthClientRequest, OAuthClient, OAuthClientResponse, OAuthClientRow,
    UpdateOAuthClientRequest,
};

fn create_test_client_row() -> OAuthClientRow {
    OAuthClientRow {
        client_id: ClientId::new("client_test123"),
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
fn test_create_oauth_client_request_deserialization() {
    let json = r#"{
        "client_id": "client_new",
        "name": "New Client",
        "redirect_uris": ["https://example.com/callback"],
        "scopes": ["openid", "profile"]
    }"#;

    let request: CreateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.client_id.as_str(), "client_new");
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

#[test]
fn test_update_oauth_client_request_full() {
    let json = r#"{
        "name": "Updated Name",
        "redirect_uris": ["https://example.com/new-callback"],
        "scopes": ["openid", "email"]
    }"#;

    let request: UpdateOAuthClientRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.name, Some("Updated Name".to_string()));
    request
        .redirect_uris
        .as_ref()
        .expect("redirect_uris should be present");
    request.scopes.as_ref().expect("scopes should be present");
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

#[test]
fn test_oauth_client_response_from_client() {
    let row = create_test_client_row();
    let relations = create_test_relations();
    let client = OAuthClient::from_row_with_relations(row, relations);

    let response: OAuthClientResponse = client.into();

    assert_eq!(response.client_id.as_str(), "client_test123");
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
