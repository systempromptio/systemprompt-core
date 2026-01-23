//! Integration tests for WebAuthn credential management

use crate::{cleanup_test_user, create_test_user, setup_test_db};
use systemprompt_oauth::repository::{OAuthRepository, WebAuthnCredentialParams};
use uuid::Uuid;

#[tokio::test]
async fn test_webauthn_credential_lifecycle() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let id = Uuid::new_v4().to_string();
    let credential_id = Uuid::new_v4().as_bytes().to_vec();
    let public_key = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let counter = 0u32;
    let transports = vec!["internal".to_string()];

    let params = WebAuthnCredentialParams::builder(&id, user_id.as_str(), &credential_id, &public_key, counter)
        .with_display_name("My Authenticator")
        .with_device_type("platform")
        .with_transports(&transports)
        .build();

    repo.store_webauthn_credential(params)
        .await
        .expect("Failed to store credential");

    let credentials = repo
        .get_webauthn_credentials(user_id.as_str())
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 1);
    let cred = &credentials[0];
    assert_eq!(cred.user_id.as_str(), user_id.as_str());
    assert_eq!(cred.credential_id, credential_id);
    assert_eq!(cred.public_key, public_key);
    assert_eq!(cred.counter, counter);
    assert_eq!(cred.display_name, "My Authenticator");

    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_webauthn_credential_counter_update() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let id = Uuid::new_v4().to_string();
    let credential_id = Uuid::new_v4().as_bytes().to_vec();
    let public_key = vec![1, 2, 3];
    let transports = vec!["usb".to_string()];

    let params = WebAuthnCredentialParams::builder(&id, user_id.as_str(), &credential_id, &public_key, 0)
        .with_display_name("Test Credential")
        .with_device_type("cross-platform")
        .with_transports(&transports)
        .build();

    repo.store_webauthn_credential(params)
        .await
        .expect("Failed to store credential");

    let new_counter = 42u32;
    repo.update_webauthn_credential_counter(&credential_id, new_counter)
        .await
        .expect("Failed to update counter");

    let credentials = repo
        .get_webauthn_credentials(user_id.as_str())
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].counter, new_counter);

    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_webauthn_multiple_credentials_per_user() {
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let repo = OAuthRepository::new(db.clone()).expect("Failed to create repository");

    let transports = vec!["internal".to_string()];

    for i in 0..3 {
        let id = Uuid::new_v4().to_string();
        let credential_id = format!("credential_{}", i).as_bytes().to_vec();
        let public_key = vec![i as u8];
        let display_name = format!("Authenticator {}", i);

        let params = WebAuthnCredentialParams::builder(&id, user_id.as_str(), &credential_id, &public_key, 0)
            .with_display_name(&display_name)
            .with_device_type("platform")
            .with_transports(&transports)
            .build();

        repo.store_webauthn_credential(params)
            .await
            .expect("Failed to store credential");
    }

    let credentials = repo
        .get_webauthn_credentials(user_id.as_str())
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 3);

    cleanup_test_user(&db, &user_id).await;
}

#[tokio::test]
async fn test_webauthn_empty_for_new_user() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(db).expect("Failed to create repository");

    let nonexistent_user_id = Uuid::new_v4().to_string();

    let credentials = repo
        .get_webauthn_credentials(&nonexistent_user_id)
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 0);
}
