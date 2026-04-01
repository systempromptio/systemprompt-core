//! Tests for OAuthClientRow and ClientRelations

use chrono::Utc;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::{ClientRelations, OAuthClientRow};

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
fn test_oauth_client_row_creation() {
    let row = create_test_client_row();
    assert_eq!(row.client_id.as_str(), "client_test123");
    assert_eq!(row.client_name, "Test Client");
    assert!(row.is_active.unwrap());
}

#[test]
fn test_oauth_client_row_with_none_values() {
    let row = OAuthClientRow {
        client_id: ClientId::new("client_minimal"),
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

    assert_eq!(row.client_id.as_str(), "client_minimal");
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
    assert_eq!(row.client_id.as_str(), "client_deser");
    assert_eq!(row.client_name, "Deserialized Client");
    assert!(row.is_active.unwrap());
}

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
