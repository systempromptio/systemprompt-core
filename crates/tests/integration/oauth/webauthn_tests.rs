use super::setup_test_pool;
use systemprompt_core_oauth::repository::{OAuthRepository, WebAuthnCredentialParams};
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn test_webauthn_credential_lifecycle() {
    let pool = setup_test_pool().await;
    let repo = OAuthRepository::new(pool).unwrap();

    let id = Uuid::new_v4().to_string();
    let credential_id = Uuid::new_v4().to_vec();
    let user_id = Uuid::new_v4().to_string();
    let public_key = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let counter = 0;
    let transports = vec!["internal".to_string()];

    let params = WebAuthnCredentialParams::builder(&id, &user_id, &credential_id, &public_key, counter)
        .with_display_name("My Authenticator")
        .with_device_type("platform")
        .with_transports(&transports)
        .build();

    repo.store_webauthn_credential(params)
        .await
        .expect("Failed to store credential");

    let credentials = repo
        .get_webauthn_credentials(&user_id)
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 1);
    let cred = &credentials[0];
    assert_eq!(cred.user_id, user_id);
    assert_eq!(cred.credential_id, credential_id);
    assert_eq!(cred.public_key, public_key);
    assert_eq!(cred.counter, counter);
    assert_eq!(cred.display_name, "My Authenticator");
}

#[tokio::test]
#[ignore]
async fn test_webauthn_credential_counter_update() {
    let pool = setup_test_pool().await;
    let repo = OAuthRepository::new(pool).unwrap();

    let id = Uuid::new_v4().to_string();
    let credential_id = Uuid::new_v4().to_vec();
    let user_id = Uuid::new_v4().to_string();
    let public_key = vec![1, 2, 3];
    let transports = vec!["usb".to_string()];

    let params = WebAuthnCredentialParams::builder(&id, &user_id, &credential_id, &public_key, 0)
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
        .get_webauthn_credentials(&user_id)
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0].counter, new_counter);
}

#[tokio::test]
#[ignore]
async fn test_webauthn_multiple_credentials_per_user() {
    let pool = setup_test_pool().await;
    let repo = OAuthRepository::new(pool).unwrap();

    let user_id = Uuid::new_v4().to_string();
    let transports = vec!["internal".to_string()];

    for i in 0..3 {
        let id = Uuid::new_v4().to_string();
        let credential_id = format!("credential_{}", i);
        let credential_bytes = credential_id.as_bytes().to_vec();
        let public_key = vec![i as u8];
        let display_name = format!("Authenticator {}", i);

        let params = WebAuthnCredentialParams::builder(&id, &user_id, &credential_bytes, &public_key, 0)
            .with_display_name(&display_name)
            .with_device_type("platform")
            .with_transports(&transports)
            .build();

        repo.store_webauthn_credential(params)
            .await
            .expect("Failed to store credential");
    }

    let credentials = repo
        .get_webauthn_credentials(&user_id)
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 3);

    for (i, cred) in credentials.iter().enumerate() {
        assert_eq!(cred.display_name, format!("Authenticator {}", i));
    }
}

#[tokio::test]
#[ignore]
async fn test_webauthn_empty_for_new_user() {
    let pool = setup_test_pool().await;
    let repo = OAuthRepository::new(pool).unwrap();

    let user_id = Uuid::new_v4().to_string();

    let credentials = repo
        .get_webauthn_credentials(&user_id)
        .await
        .expect("Failed to get credentials");

    assert_eq!(credentials.len(), 0);
}
