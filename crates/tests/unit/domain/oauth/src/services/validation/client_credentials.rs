//! Tests for client credential verification
//!
//! Tests `verify_client_authentication` which handles auth_method dispatch,
//! bcrypt secret verification, and timing-safe error paths.

use systemprompt_oauth::services::verify_client_authentication;

mod db_backed {
    use systemprompt_identifiers::ClientId;
    use systemprompt_oauth::repository::{ClientRepository, CreateClientParams, OAuthRepository};
    use systemprompt_oauth::services::hash_client_secret;
    use systemprompt_oauth::services::validation::validate_client_credentials;
    use systemprompt_test_fixtures::{
        ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
    };
    use uuid::Uuid;

    const SECRET: &str = "client-credentials-secret-32-chars!!";

    async fn seeded_client() -> Option<(OAuthRepository, ClientId)> {
        let url = fixture_database_url().ok()?;
        ensure_test_bootstrap();
        let pool = fixture_db_pool(&url).await.expect("pool");
        let owner = unique_user_id("cc");
        seed_user_row(&pool, &owner, &format!("{}@cc.invalid", owner.as_str()))
            .await
            .expect("seed owner");
        let client_id = ClientId::new(format!("client_{}", Uuid::new_v4().simple()));
        ClientRepository::new(&pool)
            .expect("client repo")
            .create(CreateClientParams {
                client_id: client_id.clone(),
                owner_user_id: owner,
                client_secret_hash: hash_client_secret(SECRET).expect("hash"),
                client_name: "cc-test".to_owned(),
                redirect_uris: vec!["http://127.0.0.1/cb".to_owned()],
                grant_types: Some(vec!["client_credentials".to_owned()]),
                response_types: Some(vec!["code".to_owned()]),
                scopes: vec!["openid".to_owned()],
                token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
                application_type: "web".to_owned(),
                client_uri: None,
                logo_uri: None,
                contacts: None,
            })
            .await
            .expect("create client");
        let repo = OAuthRepository::new(&pool).expect("repo");
        Some((repo, client_id))
    }

    #[tokio::test]
    async fn correct_secret_authenticates_registered_client() {
        let Some((repo, client_id)) = seeded_client().await else {
            return;
        };
        validate_client_credentials(&repo, &client_id, Some(SECRET))
            .await
            .expect("correct secret authenticates");
    }

    #[tokio::test]
    async fn wrong_secret_is_rejected() {
        let Some((repo, client_id)) = seeded_client().await else {
            return;
        };
        let err = validate_client_credentials(&repo, &client_id, Some("wrong-secret"))
            .await
            .expect_err("wrong secret");
        assert!(err.to_string().contains("Invalid client secret"));
    }

    #[tokio::test]
    async fn missing_secret_is_rejected() {
        let Some((repo, client_id)) = seeded_client().await else {
            return;
        };
        let err = validate_client_credentials(&repo, &client_id, None)
            .await
            .expect_err("missing secret");
        assert!(err.to_string().contains("Client secret required"));
    }

    #[tokio::test]
    async fn unknown_client_is_rejected() {
        let Some((repo, _client_id)) = seeded_client().await else {
            return;
        };
        let missing = ClientId::new(format!("client_{}", Uuid::new_v4().simple()));
        let err = validate_client_credentials(&repo, &missing, Some(SECRET))
            .await
            .expect_err("unknown client");
        assert!(err.to_string().contains("Client not found"));
    }
}

#[test]
fn auth_method_none_returns_ok() {
    verify_client_authentication("none", None, None).expect("auth_method none passes");
}

#[test]
fn auth_method_none_ignores_secret_and_hash() {
    let hash = bcrypt::hash("some_secret", 4).unwrap();

    verify_client_authentication("none", Some(&hash), Some("some_secret"))
        .expect("auth_method none ignores secret and hash");
}

#[test]
fn valid_secret_and_hash_returns_ok() {
    let hash = bcrypt::hash("test_secret", 4).unwrap();

    verify_client_authentication("client_secret_post", Some(&hash), Some("test_secret"))
        .expect("valid secret and hash pass");
}

#[test]
fn client_secret_post_method_with_valid_creds() {
    let secret = "my_application_secret_value";
    let hash = bcrypt::hash(secret, 4).unwrap();

    verify_client_authentication("client_secret_post", Some(&hash), Some(secret))
        .expect("client_secret_post with valid creds passes");
}

#[test]
fn wrong_secret_returns_err() {
    let hash = bcrypt::hash("correct_secret", 4).unwrap();

    let result =
        verify_client_authentication("client_secret_post", Some(&hash), Some("wrong_secret"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Invalid client secret"),
        "Expected 'Invalid client secret' but got: {err_msg}"
    );
}

#[test]
fn hash_present_no_secret_returns_err() {
    let hash = bcrypt::hash("test_secret", 4).unwrap();

    let result = verify_client_authentication("client_secret_post", Some(&hash), None);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client secret required"),
        "Expected 'Client secret required' but got: {err_msg}"
    );
}

#[test]
fn no_hash_secret_present_returns_err() {
    let result = verify_client_authentication("client_secret_post", None, Some("orphan_secret"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client has no secret hash configured"),
        "Expected 'Client has no secret hash configured' but got: {err_msg}"
    );
}

#[test]
fn no_hash_no_secret_returns_err() {
    let result = verify_client_authentication("client_secret_post", None, None);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Client secret required"),
        "Expected 'Client secret required' but got: {err_msg}"
    );
}

#[test]
fn error_messages_are_descriptive() {
    let hash = bcrypt::hash("secret", 4).unwrap();

    let no_secret_err = verify_client_authentication("client_secret_basic", Some(&hash), None)
        .unwrap_err()
        .to_string();
    assert!(no_secret_err.contains("Client secret required"));

    let no_hash_err = verify_client_authentication("client_secret_basic", None, Some("secret"))
        .unwrap_err()
        .to_string();
    assert!(no_hash_err.contains("Client has no secret hash configured"));

    let wrong_secret_err =
        verify_client_authentication("client_secret_basic", Some(&hash), Some("wrong"))
            .unwrap_err()
            .to_string();
    assert!(wrong_secret_err.contains("Invalid client secret"));
}

#[test]
fn various_auth_methods_require_secret() {
    let methods = [
        "client_secret_basic",
        "client_secret_post",
        "client_secret_jwt",
    ];

    for method in methods {
        let result = verify_client_authentication(method, None, None);
        assert!(
            result.is_err(),
            "auth_method '{method}' should require credentials"
        );
    }
}
