//! Tests for OAuthClient construction and validation

use chrono::Utc;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::{ClientRelations, OAuthClient, OAuthClientRow};

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
fn test_oauth_client_from_row_with_relations() {
    let row = create_test_client_row();
    let relations = create_test_relations();

    let client = OAuthClient::from_row_with_relations(row, relations);

    assert_eq!(client.client_id.as_str(), "client_test123");
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
        client_id: ClientId::new("client_defaults"),
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
    row.client_id = ClientId::new("");
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
