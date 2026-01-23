//! Integration tests for OAuth client repository

use crate::setup_test_db;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams, UpdateClientParams};
use uuid::Uuid;

fn test_client_id() -> ClientId {
    ClientId::new(&format!("test_client_{}", Uuid::new_v4()))
}

#[tokio::test]
async fn test_client_lifecycle() {
    let db = setup_test_db().await;
    let repo = ClientRepository::new(&db).expect("Failed to create repository");

    let client_id = test_client_id();
    let redirect_uris = vec!["http://localhost:3000/callback".to_string()];
    let grant_types = vec!["authorization_code".to_string(), "refresh_token".to_string()];
    let response_types = vec!["code".to_string()];
    let scopes = vec!["openid".to_string(), "profile".to_string()];

    let params = CreateClientParams {
        client_id: client_id.clone(),
        client_secret_hash: "hash_of_secret".to_string(),
        client_name: "Test Client".to_string(),
        redirect_uris: redirect_uris.clone(),
        grant_types: Some(grant_types),
        response_types: Some(response_types),
        scopes: scopes.clone(),
        token_endpoint_auth_method: Some("client_secret_post".to_string()),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    let created = repo.create(params).await.expect("Failed to create client");

    assert_eq!(created.client_id.as_str(), client_id.as_str());
    assert!(created.is_active);
    assert_eq!(created.scopes.len(), 2);
    assert_eq!(created.redirect_uris.len(), 1);

    let found = repo
        .get_by_client_id(client_id.as_str())
        .await
        .expect("Failed to get client")
        .expect("Client not found");

    assert_eq!(found.client_id.as_str(), created.client_id.as_str());
    assert_eq!(found.scopes, created.scopes);

    repo.deactivate(client_id.as_str())
        .await
        .expect("Failed to deactivate client");

    let inactive = repo
        .get_by_client_id(client_id.as_str())
        .await
        .expect("Failed to get client");

    assert!(inactive.is_none(), "Deactivated client should not be found");

    let found_any = repo
        .get_by_client_id_any(client_id.as_str())
        .await
        .expect("Failed to get client (any)")
        .expect("Client not found (any)");

    assert!(!found_any.is_active, "Client should be inactive");

    repo.activate(client_id.as_str())
        .await
        .expect("Failed to activate client");

    let reactivated = repo
        .get_by_client_id(client_id.as_str())
        .await
        .expect("Failed to get client")
        .expect("Client not found after reactivation");

    assert!(reactivated.is_active, "Client should be active again");

    repo.delete(client_id.as_str())
        .await
        .expect("Failed to delete client");

    let deleted = repo
        .get_by_client_id(client_id.as_str())
        .await
        .expect("Failed to query deleted client");

    assert!(deleted.is_none(), "Deleted client should not be found");
}

#[tokio::test]
async fn test_client_update() {
    let db = setup_test_db().await;
    let repo = ClientRepository::new(&db).expect("Failed to create repository");

    let client_id = test_client_id();
    let original_scopes = vec!["openid".to_string()];
    let new_scopes = vec!["openid".to_string(), "email".to_string()];
    let new_uris = vec!["http://localhost:4000/callback".to_string()];

    let create_params = CreateClientParams {
        client_id: client_id.clone(),
        client_secret_hash: "hash_of_secret".to_string(),
        client_name: "Original Name".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: None,
        response_types: None,
        scopes: original_scopes,
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    repo.create(create_params)
        .await
        .expect("Failed to create client");

    let update_params = UpdateClientParams {
        client_id: client_id.clone(),
        client_name: "Updated Name".to_string(),
        redirect_uris: new_uris.clone(),
        grant_types: None,
        response_types: None,
        scopes: new_scopes.clone(),
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    let updated = repo
        .update(update_params)
        .await
        .expect("Failed to update client")
        .expect("Client not found after update");

    assert_eq!(updated.client_name, "Updated Name");
    assert_eq!(updated.scopes.len(), 2);
    assert!(updated.scopes.contains(&"email".to_string()));
    assert_eq!(updated.redirect_uris[0], "http://localhost:4000/callback");

    repo.delete(client_id.as_str()).await.ok();
}

#[tokio::test]
async fn test_client_secret_update() {
    let db = setup_test_db().await;
    let repo = ClientRepository::new(&db).expect("Failed to create repository");

    let client_id = test_client_id();

    let create_params = CreateClientParams {
        client_id: client_id.clone(),
        client_secret_hash: "original_hash".to_string(),
        client_name: "Test Client".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: None,
        response_types: None,
        scopes: vec!["openid".to_string()],
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    repo.create(create_params)
        .await
        .expect("Failed to create client");

    let updated = repo
        .update_secret(client_id.as_str(), "new_hash")
        .await
        .expect("Failed to update secret")
        .expect("Client not found");

    assert_eq!(updated.client_secret_hash, Some("new_hash".to_string()));

    repo.delete(client_id.as_str()).await.ok();
}

#[tokio::test]
async fn test_client_counting() {
    let db = setup_test_db().await;
    let repo = ClientRepository::new(&db).expect("Failed to create repository");

    let client_id = test_client_id();

    let initial_count = repo.count().await.expect("Failed to count clients");
    assert!(initial_count >= 0, "Count should return a non-negative value");

    let create_params = CreateClientParams {
        client_id: client_id.clone(),
        client_secret_hash: "hash".to_string(),
        client_name: "Count Test".to_string(),
        redirect_uris: vec!["http://localhost:3000/callback".to_string()],
        grant_types: None,
        response_types: None,
        scopes: vec!["openid".to_string()],
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    repo.create(create_params)
        .await
        .expect("Failed to create client");

    let count_after_create = repo.count().await.expect("Failed to count clients");
    assert!(count_after_create > 0, "Count should be positive after creating client");

    let client_exists = repo
        .get_by_client_id(client_id.as_str())
        .await
        .expect("Failed to query client")
        .is_some();
    assert!(client_exists, "Created client should be retrievable");

    repo.delete(client_id.as_str())
        .await
        .expect("Failed to delete client");

    let count_after_delete = repo.count().await.expect("Failed to count after delete");
    assert!(count_after_delete >= 0, "Count should be non-negative after delete");
}
